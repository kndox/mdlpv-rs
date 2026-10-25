local M = {}

M.defaults = {
  server_bin = "mdlpv-rs",
  host = "127.0.0.1",
  port = 0,
  server_log_level = "warn",
  server_stderr_notify = false,
  debounce_ms = 250,
  scroll_sync = true,
  scroll_debounce_ms = 50,
  open_browser = true,
  browser_cmd = nil,
  pdf_browser_cmd = nil,
  pdf_virtual_time_budget_ms = 5000,
  pdf_extra_args = {},
  mermaid = {
    enabled = true,
  },
}

M.options = vim.deepcopy(M.defaults)

local function merge(defaults, opts)
  return vim.tbl_deep_extend("force", defaults, opts or {})
end

local warned_mermaid_options = false

function M.setup(opts)
  local mermaid = opts and opts.mermaid
  if mermaid and (mermaid.mode ~= nil or mermaid.cdn_url ~= nil) and not warned_mermaid_options then
    warned_mermaid_options = true
    vim.schedule(function()
      vim.notify("mdlive 0.2.0: mermaid.mode and mermaid.cdn_url were removed; use mermaid.enabled. Diagrams now render with merman.", vim.log.levels.WARN)
    end)
  end
  M.options = merge(vim.deepcopy(M.defaults), opts)
  M.options.mermaid.mode = nil
  M.options.mermaid.cdn_url = nil
  return M.options
end

return M
