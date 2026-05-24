local M = {}

local pdf_browser_candidates = {
  "chromium",
  "chromium-browser",
  "google-chrome",
  "google-chrome-stable",
  "microsoft-edge",
  "brave-browser",
}

local function split_cmd(cmd)
  if type(cmd) == "table" then
    return vim.deepcopy(cmd)
  end
  return vim.split(cmd, "%s+", { trimempty = true })
end

local function find_pdf_browser(opts)
  if opts.pdf_browser_cmd and opts.pdf_browser_cmd ~= "" then
    return split_cmd(opts.pdf_browser_cmd)
  end

  for _, candidate in ipairs(pdf_browser_candidates) do
    if vim.fn.executable(candidate) == 1 then
      return { candidate }
    end
  end

  return nil
end

function M.open(url, opts)
  local cmd
  if opts.browser_cmd then
    cmd = split_cmd(opts.browser_cmd)
    table.insert(cmd, url)
  elseif vim.fn.has("mac") == 1 then
    cmd = { "open", url }
  elseif vim.fn.has("win32") == 1 then
    cmd = { "cmd", "/c", "start", "", url }
  else
    cmd = { "xdg-open", url }
  end

  vim.system(cmd, { text = true }, function(result)
    if result.code ~= 0 then
      vim.schedule(function()
        vim.notify("mdlive: failed to open browser", vim.log.levels.ERROR)
      end)
    end
  end)
end

function M.export_pdf(url, output_path, opts, cb)
  local cmd = find_pdf_browser(opts)
  if not cmd then
    cb("no supported headless browser found for PDF export")
    return
  end

  table.insert(cmd, "--headless")
  table.insert(cmd, "--disable-gpu")
  table.insert(cmd, "--no-sandbox")
  table.insert(cmd, "--virtual-time-budget=" .. tostring(opts.pdf_virtual_time_budget_ms))
  for _, arg in ipairs(opts.pdf_extra_args or {}) do
    table.insert(cmd, arg)
  end
  table.insert(cmd, "--print-to-pdf=" .. output_path)
  table.insert(cmd, url)

  vim.system(cmd, { text = true }, function(result)
    if result.code ~= 0 then
      local stderr = result.stderr or ""
      local stdout = result.stdout or ""
      local message = stderr ~= "" and stderr or stdout
      cb(message ~= "" and message or "PDF export failed")
      return
    end
    cb(nil)
  end)
end

return M
