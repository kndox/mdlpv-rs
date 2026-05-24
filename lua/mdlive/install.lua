local M = {}

local OWNER = "kndox"
local REPO = "mdlpv-rs"

local ASSETS = {
  linux = {
    x86_64 = "mdlpv-rs-linux-x86_64.tar.gz",
    aarch64 = "mdlpv-rs-linux-aarch64.tar.gz",
  },
  macos = {
    x86_64 = "mdlpv-rs-macos-x86_64.tar.gz",
    aarch64 = "mdlpv-rs-macos-aarch64.tar.gz",
  },
  windows = {
    x86_64 = "mdlpv-rs-windows-x86_64.zip",
    aarch64 = "mdlpv-rs-windows-aarch64.zip",
  },
}

local function normalize_os(sysname)
  sysname = (sysname or ""):lower()
  if sysname:match("linux") then
    return "linux"
  end
  if sysname:match("darwin") then
    return "macos"
  end
  if sysname:match("windows") or sysname:match("mingw") or sysname:match("msys") then
    return "windows"
  end
  return nil
end

local function normalize_arch(machine)
  machine = (machine or ""):lower()
  if machine == "x86_64" or machine == "amd64" then
    return "x86_64"
  end
  if machine == "aarch64" or machine == "arm64" then
    return "aarch64"
  end
  return nil
end

local function run(args, opts)
  opts = opts or {}
  local result = vim.system(args, { text = true, cwd = opts.cwd }):wait()
  if result.code ~= 0 then
    local output = vim.trim((result.stderr or "") .. "\n" .. (result.stdout or ""))
    error((opts.label or args[1]) .. " failed: " .. output)
  end
  return result.stdout or ""
end

local function executable(name)
  return vim.fn.executable(name) == 1
end

local function download(url, output_path)
  if not executable("curl") then
    error("curl is required to install mdlpv-rs prebuilt binary")
  end
  run({
    "curl",
    "--fail",
    "--location",
    "--show-error",
    "--silent",
    "--retry",
    "3",
    "--output",
    output_path,
    url,
  }, { label = "download " .. url })
end

local function parse_checksums(lines)
  local checksums = {}
  for _, line in ipairs(lines) do
    local hash, name = line:match("^%s*([A-Fa-f0-9]+)%s+%*?(.+)%s*$")
    if hash and name then
      checksums[vim.fn.fnamemodify(name, ":t")] = hash:lower()
    end
  end
  return checksums
end

local function checksum_with_powershell(path)
  local shell = executable("powershell") and "powershell" or (executable("pwsh") and "pwsh" or nil)
  if not shell then
    return nil
  end
  return vim.trim(run({
    shell,
    "-NoProfile",
    "-Command",
    "(Get-FileHash -Algorithm SHA256 -Path $args[0]).Hash.ToLowerInvariant()",
    path,
  }, { label = "checksum" }))
end

local function checksum(path)
  if executable("sha256sum") then
    local output = run({ "sha256sum", path }, { label = "checksum" })
    return (output:match("^%s*([A-Fa-f0-9]+)") or ""):lower()
  end
  if executable("shasum") then
    local output = run({ "shasum", "-a", "256", path }, { label = "checksum" })
    return (output:match("^%s*([A-Fa-f0-9]+)") or ""):lower()
  end
  local powershell_hash = checksum_with_powershell(path)
  if powershell_hash then
    return powershell_hash
  end
  error("sha256sum, shasum, or PowerShell is required to verify mdlpv-rs archive")
end

local function extract(archive_path, extract_dir, os_name)
  vim.fn.mkdir(extract_dir, "p")
  if archive_path:match("%.zip$") then
    if os_name == "windows" and executable("powershell") then
      run({
        "powershell",
        "-NoProfile",
        "-Command",
        "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
        archive_path,
        extract_dir,
      }, { label = "extract" })
      return
    end
    if os_name == "windows" and executable("pwsh") then
      run({
        "pwsh",
        "-NoProfile",
        "-Command",
        "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
        archive_path,
        extract_dir,
      }, { label = "extract" })
      return
    end
    if executable("unzip") then
      run({ "unzip", "-q", "-o", archive_path, "-d", extract_dir }, { label = "extract" })
      return
    end
    error("unzip or PowerShell is required to extract " .. archive_path)
  end
  if not executable("tar") then
    error("tar is required to extract " .. archive_path)
  end
  run({ "tar", "-xzf", archive_path, "-C", extract_dir }, { label = "extract" })
end

local function copy_file(source, target)
  local ok, err = (vim.uv or vim.loop).fs_copyfile(source, target)
  if not ok then
    error("failed to install binary to " .. target .. ": " .. tostring(err))
  end
end

function M.detect_asset(uname)
  uname = uname or (vim.uv or vim.loop).os_uname()
  local os_name = normalize_os(uname.sysname)
  local arch = normalize_arch(uname.machine)
  if not os_name or not arch or not ASSETS[os_name] or not ASSETS[os_name][arch] then
    error("unsupported platform for mdlpv-rs prebuilt binary: " .. tostring(uname.sysname) .. " " .. tostring(uname.machine))
  end
  return ASSETS[os_name][arch], os_name
end

function M.binary_name(os_name)
  return os_name == "windows" and "mdlpv-rs.exe" or "mdlpv-rs"
end

function M.checksum_for(lines, asset)
  return parse_checksums(lines)[asset]
end

function M.install(plugin)
  if not plugin or not plugin.dir then
    error("mdlpv-rs installer requires Lazy.nvim plugin.dir")
  end

  local asset, os_name = M.detect_asset()
  local base_url = string.format("https://github.com/%s/%s/releases/latest/download", OWNER, REPO)
  local tmp_dir = vim.fn.tempname()
  local archive_path = tmp_dir .. "/" .. asset
  local sums_path = tmp_dir .. "/SHA256SUMS"
  local extract_dir = tmp_dir .. "/extract"
  local binary_name = M.binary_name(os_name)
  local extracted_binary = extract_dir .. "/mdlpv-rs/" .. binary_name
  local target_binary = plugin.dir .. "/" .. binary_name

  vim.fn.mkdir(tmp_dir, "p")
  local ok, err = pcall(function()
    download(base_url .. "/" .. asset, archive_path)
    download(base_url .. "/SHA256SUMS", sums_path)

    local expected = M.checksum_for(vim.fn.readfile(sums_path), asset)
    if not expected then
      error("SHA256SUMS does not contain checksum for " .. asset)
    end
    local actual = checksum(archive_path)
    if actual ~= expected then
      error(string.format("checksum mismatch for %s: expected %s, got %s", asset, expected, actual))
    end

    extract(archive_path, extract_dir, os_name)
    if vim.fn.executable(extracted_binary) ~= 1 and vim.fn.filereadable(extracted_binary) ~= 1 then
      error("release archive did not contain " .. binary_name)
    end
    vim.fn.delete(target_binary)
    copy_file(extracted_binary, target_binary)
    vim.fn.delete(extracted_binary)
    if os_name ~= "windows" then
      vim.fn.setfperm(target_binary, "rwxr-xr-x")
    end
  end)
  vim.fn.delete(tmp_dir, "rf")
  if not ok then
    error(err)
  end

  vim.notify("mdlive: installed " .. asset, vim.log.levels.INFO)
  return target_binary
end

return M
