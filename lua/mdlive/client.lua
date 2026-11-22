local config = require("mdlive.config")

local M = {}

M.state = {
  process = nil,
  -- Identifies the current server launch so callbacks from a stopped process are ignored.
  generation = nil,
  host = nil,
  port = nil,
  -- File holding the Authorization header, so the token never appears in curl's argv.
  auth_header_path = nil,
  starting = false,
  waiters = {},
}

local uv = vim.uv or vim.loop

local function notify(message, level)
  vim.schedule(function()
    vim.notify(message, level or vim.log.levels.ERROR)
  end)
end

local function schedule_cb(cb, ...)
  local args = vim.F.pack_len(...)
  vim.schedule(function()
    cb(vim.F.unpack_len(args))
  end)
end

local function url_host(host)
  if host:find(":", 1, true) and host:sub(1, 1) ~= "[" then
    return "[" .. host .. "]"
  end
  return host
end

local function base_url()
  return string.format("http://%s:%s", url_host(M.state.host), M.state.port)
end

-- Runs in a fast event context, so only libuv file APIs are used.
local function write_auth_header(path, token)
  local fd, err = uv.fs_open(path, "w", tonumber("600", 8))
  if not fd then
    return err
  end
  local _, write_err = uv.fs_write(fd, "Authorization: Bearer " .. token .. "\n")
  uv.fs_close(fd)
  return write_err
end

local function remove_file(path)
  if path then
    os.remove(path)
  end
end

local function flush_waiters(err)
  local waiters = M.state.waiters
  M.state.waiters = {}
  for _, cb in ipairs(waiters) do
    vim.schedule(function()
      cb(err)
    end)
  end
end

function M.ensure_server(cb)
  if M.state.host and M.state.port then
    schedule_cb(cb, nil)
    return
  end

  table.insert(M.state.waiters, cb)
  if M.state.starting then
    return
  end
  M.state.starting = true

  local opts = config.options
  local args = {
    opts.server_bin,
    "--host",
    opts.host,
    "--port",
    tostring(opts.port),
    "--log-level",
    tostring(opts.server_log_level or "warn"),
    "--mermaid-mode",
    opts.mermaid.enabled == false and "none" or "merman",
  }

  local generation = {}
  local auth_header_path = vim.fn.tempname()
  M.state.generation = generation
  M.state.auth_header_path = auth_header_path
  local function is_current()
    return M.state.generation == generation
  end

  local stdout = ""
  local stderr = ""
  local ready = false
  local process = vim.system(args, {
    text = true,
    stdout = function(_, data)
      if not data or ready or not is_current() then
        return
      end
      stdout = stdout .. data
      -- Wait for a complete line: stdout may arrive in partial chunks.
      local line = stdout:match("^([^\r\n]*)\r?\n")
      if not line then
        return
      end
      ready = true
      local ok, decoded = pcall(vim.json.decode, line)
      if not (ok and type(decoded) == "table" and decoded.host and decoded.port and decoded.token) then
        flush_waiters("server did not report its address and API token; update the mdlpv-rs binary")
        M.stop_server()
        return
      end
      local write_err = write_auth_header(auth_header_path, decoded.token)
      if write_err then
        flush_waiters("failed to store server token: " .. tostring(write_err))
        M.stop_server()
        return
      end
      M.state.host = decoded.host
      M.state.port = decoded.port
      M.state.starting = false
      flush_waiters(nil)
    end,
    stderr = function(_, data)
      if data and data:match("%S") then
        if not ready then
          stderr = stderr .. data
        end
        if opts.server_stderr_notify then
          vim.schedule(function()
            vim.notify("mdlive server: " .. vim.trim(data), vim.log.levels.DEBUG)
          end)
        end
      end
    end,
  }, function(result)
    remove_file(auth_header_path)
    if not is_current() then
      return
    end
    M.state.process = nil
    M.state.generation = nil
    M.state.host = nil
    M.state.port = nil
    M.state.auth_header_path = nil
    M.state.starting = false
    if not ready then
      local message = "server exited before startup: " .. tostring(result.code)
      local trimmed_stderr = vim.trim(stderr)
      if trimmed_stderr ~= "" then
        message = message .. ": " .. trimmed_stderr
      end
      flush_waiters(message)
    end
  end)
  if is_current() then
    M.state.process = process
  end
end

function M.stop_server()
  if M.state.process then
    pcall(function()
      M.state.process:kill(15)
    end)
  end
  remove_file(M.state.auth_header_path)
  M.state.process = nil
  M.state.generation = nil
  M.state.host = nil
  M.state.port = nil
  M.state.auth_header_path = nil
  M.state.starting = false
  -- Fail pending starts instead of dropping them, so their callers can recover.
  flush_waiters("server stopped")
end

function M.generation()
  return M.state.generation
end

function M.request(method, path, body, cb)
  M.request_raw(method, path, body, function(err, stdout)
    if err then
      cb(err)
      return
    end
    local ok, decoded = pcall(vim.json.decode, stdout)
    if not ok then
      cb("invalid JSON response: " .. stdout)
      return
    end
    cb(nil, decoded)
  end)
end

function M.request_raw(method, path, body, cb)
  if not M.state.host then
    schedule_cb(cb, "server is not running")
    return
  end

  local args = {
    "curl",
    "--silent",
    "--show-error",
    "--fail-with-body",
    "--max-time",
    "60",
    "--request",
    method,
    "--header",
    "Content-Type: application/json",
    "--header",
    "@" .. M.state.auth_header_path,
  }
  local stdin = nil
  if body then
    stdin = vim.json.encode(body)
    table.insert(args, "--data-binary")
    table.insert(args, "@-")
  end
  table.insert(args, base_url() .. path)

  vim.system(args, { text = true, stdin = stdin }, function(result)
    if result.code ~= 0 then
      local stderr = result.stderr or ""
      local stdout = result.stdout or ""
      schedule_cb(cb, stderr ~= "" and stderr or stdout)
      return
    end
    schedule_cb(cb, nil, result.stdout)
  end)
end

function M.base_url()
  return base_url()
end

function M.notify_error(message)
  notify("mdlive: " .. message, vim.log.levels.ERROR)
end

return M
