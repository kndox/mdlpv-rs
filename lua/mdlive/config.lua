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
    mode = "local",
    cdn_url = "https://cdn.jsdelivr.net/npm/mermaid@11.15.0/dist/mermaid.min.js",
  },
}

M.options = vim.deepcopy(M.defaults)

local function merge(defaults, opts)
  return vim.tbl_deep_extend("force", defaults, opts or {})
end

function M.setup(opts)
  M.options = merge(vim.deepcopy(M.defaults), opts)
  if M.options.mermaid.enabled == false then
    M.options.mermaid.mode = "none"
  end
  return M.options
end

return M
