local root = vim.fn.getcwd()
package.path = root .. "/lua/?.lua;" .. root .. "/lua/?/init.lua;" .. package.path

local calls = {}
local killed_signal = nil
local delay_session_delete = false
local delayed_delete_exit = nil
local notifications = {}

vim.notify = function(message, level)
  table.insert(notifications, { message = message, level = level })
end

vim.system = function(args, opts, on_exit)
  table.insert(calls, { args = args, opts = opts })

  if args[1] == "fail-server" then
    opts.stderr(nil, "failed\n")
    vim.schedule(function()
      on_exit({ code = 1, stdout = "", stderr = "failed" })
    end)
    return {
      kill = function(_, signal)
        killed_signal = signal
      end,
    }
  end

  if args[1] == "mdlpv-rs" then
    opts.stderr(nil, "server started\n")
    opts.stdout(nil, '{"host":"127.0.0.1","port":34567}\n')
    return {
      kill = function(_, signal)
        killed_signal = signal
      end,
    }
  end

  if args[1] == "curl" then
    local url = args[#args]
    local method = "GET"
    for index, arg in ipairs(args) do
      if arg == "--request" then
        method = args[index + 1]
      end
    end

    if url:match("/fail$") then
      vim.schedule(function()
        on_exit({ code = 22, stdout = "bad request", stderr = "" })
      end)
      return {}
    end

    local stdout = '{"ok":true}'
    if method == "POST" and url:match("/api/session$") then
      stdout = '{"session_id":"00000000-0000-4000-8000-000000000001","view_url":"http://127.0.0.1:34567/view/00000000-0000-4000-8000-000000000001"}'
    elseif method == "POST" and url:match("/api/session/") then
      stdout = '{"ok":true,"revision":2}'
    elseif method == "GET" and url:match("/api/export/") then
      stdout = "<!doctype html>\n<title>export</title>\n"
    end

    if method == "DELETE" and delay_session_delete then
      delayed_delete_exit = function()
        vim.schedule(function()
          on_exit({ code = 0, stdout = stdout, stderr = "" })
        end)
      end
      return {}
    end

    vim.schedule(function()
      on_exit({ code = 0, stdout = stdout, stderr = "" })
    end)
    return {}
  end

  if args[1] == "browser-open" then
    vim.schedule(function()
      on_exit({ code = 0, stdout = "", stderr = "" })
    end)
    return {}
  end

  if args[1] == "chromium" then
    vim.schedule(function()
      on_exit({ code = 0, stdout = "", stderr = "" })
    end)
    return {}
  end

  error("unexpected command: " .. table.concat(args, " "))
end

local function assert_true(value, message)
  if not value then
    error(message or "assertion failed", 2)
  end
end

local function wait_for(predicate, message)
  local ok = vim.wait(1000, predicate, 10)
  assert_true(ok, message)
end

local function count_mdlive_server_notifications()
  local count = 0
  for _, notification in ipairs(notifications) do
    if notification.message:match("^mdlive server:") then
      count = count + 1
    end
  end
  return count
end

local mdlive = require("mdlive")
local browser_module = require("mdlive.browser")
local client = require("mdlive.client")
local config = require("mdlive.config")
local install = require("mdlive.install")
mdlive.setup({
  open_browser = false,
  debounce_ms = 1,
  scroll_debounce_ms = 1,
})

local commands = vim.api.nvim_get_commands({})
assert_true(commands.MdLiveStart ~= nil, "MdLiveStart command is missing")
assert_true(commands.MdLiveExportHtml ~= nil, "MdLiveExportHtml command is missing")
assert_true(commands.MdLiveExportPdf ~= nil, "MdLiveExportPdf command is missing")

mdlive.setup({
  open_browser = false,
  debounce_ms = 1,
  scroll_debounce_ms = 1,
})
local leave_autocmds = vim.api.nvim_get_autocmds({ group = "mdlive", event = "VimLeavePre" })
assert_true(#leave_autocmds == 1, "setup created duplicate VimLeavePre autocmds")

local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_set_current_buf(bufnr)
vim.api.nvim_buf_set_name(bufnr, "/tmp/mdlive-test.md")
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "# Title", "", "body" })

local started = false
mdlive.start(bufnr, function(err)
  assert_true(err == nil, "start failed: " .. tostring(err))
  started = true
end)
wait_for(function()
  return started
end, "start callback did not run")
vim.wait(50)
assert_true(count_mdlive_server_notifications() == 0, "server stderr was notified by default")

local server_call = calls[1]
assert_true(vim.tbl_contains(server_call.args, "--log-level"), "server log level flag was not passed")
assert_true(vim.tbl_contains(server_call.args, "warn"), "default server log level was not warn")

config.options.server_stderr_notify = true
local notifications_before_post_start_stderr = count_mdlive_server_notifications()
server_call.opts.stderr(nil, "post-start warning\n")
wait_for(function()
  return count_mdlive_server_notifications() == notifications_before_post_start_stderr + 1
end, "server stderr after startup was not notified")
config.options.server_stderr_notify = false

local post_call = nil
for _, call in ipairs(calls) do
  if call.args[1] == "curl" and call.args[#call.args]:match("/api/session$") then
    post_call = call
  end
end
assert_true(post_call ~= nil, "session POST was not sent")
assert_true(post_call.opts.stdin:match('"content":"# Title\\n\\nbody"') ~= nil, "buffer payload was not encoded")

local function count_session_updates()
  local count = 0
  for _, call in ipairs(calls) do
    if
      call.args[1] == "curl"
      and call.args[#call.args]:match("/api/session/.+$")
      and not call.args[#call.args]:match("/scroll$")
    then
      count = count + 1
    end
  end
  return count
end

local function count_session_creates()
  local count = 0
  for _, call in ipairs(calls) do
    if call.args[1] == "curl" and call.args[#call.args]:match("/api/session$") then
      for index, arg in ipairs(call.args) do
        if arg == "--request" and call.args[index + 1] == "POST" then
          count = count + 1
        end
      end
    end
  end
  return count
end

local function count_session_deletes()
  local count = 0
  for _, call in ipairs(calls) do
    if call.args[1] == "curl" and call.args[#call.args]:match("/api/session/.+$") then
      for index, arg in ipairs(call.args) do
        if arg == "--request" and call.args[index + 1] == "DELETE" then
          count = count + 1
        end
      end
    end
  end
  return count
end

local function scroll_calls()
  local result = {}
  for _, call in ipairs(calls) do
    if call.args[1] == "curl" and call.args[#call.args]:match("/api/session/.+/scroll$") then
      table.insert(result, call)
    end
  end
  return result
end

local function count_scroll_calls()
  return #scroll_calls()
end

local function last_scroll_call()
  local result = scroll_calls()
  return result[#result]
end

local update_count = count_session_updates()
vim.api.nvim_buf_set_lines(bufnr, 1, -1, false, { "", "changed once" })
vim.api.nvim_exec_autocmds("TextChanged", { buffer = bufnr })
vim.api.nvim_buf_set_lines(bufnr, 1, -1, false, { "", "changed twice" })
vim.api.nvim_exec_autocmds("TextChanged", { buffer = bufnr })
wait_for(function()
  return count_session_updates() == update_count + 1
end, "debounced update POST was not sent once")

vim.api.nvim_win_set_cursor(0, { 3, 0 })
mdlive.scroll(bufnr)
local scroll_call = nil
wait_for(function()
  for _, call in ipairs(calls) do
    if call.args[1] == "curl" and call.args[#call.args]:match("/api/session/.+/scroll$") then
      scroll_call = call
      return true
    end
  end
  return false
end, "scroll POST was not sent")
assert_true(scroll_call.opts.stdin:match('"line":3') ~= nil, "scroll line was not encoded")

local call_count = #calls
config.options.scroll_sync = false
mdlive.scroll(bufnr)
vim.wait(50)
assert_true(#calls == call_count, "scroll POST was sent while scroll sync was disabled")
config.options.scroll_sync = true

config.options.scroll_debounce_ms = 200
local scroll_count = count_scroll_calls()
vim.api.nvim_win_set_cursor(0, { 1, 0 })
vim.api.nvim_exec_autocmds("CursorMoved", { buffer = bufnr })
wait_for(function()
  return count_scroll_calls() == scroll_count + 1
end, "first scheduled scroll POST was not sent immediately")
assert_true(last_scroll_call().opts.stdin:match('"line":1') ~= nil, "first scheduled scroll line was not encoded")

vim.api.nvim_win_set_cursor(0, { 2, 0 })
vim.api.nvim_exec_autocmds("CursorMoved", { buffer = bufnr })
vim.api.nvim_win_set_cursor(0, { 3, 0 })
vim.api.nvim_exec_autocmds("CursorMoved", { buffer = bufnr })
vim.wait(50)
assert_true(count_scroll_calls() == scroll_count + 1, "throttled scroll sent too early")
wait_for(function()
  return count_scroll_calls() == scroll_count + 2
end, "trailing scheduled scroll POST was not sent")
assert_true(last_scroll_call().opts.stdin:match('"line":3') ~= nil, "trailing scheduled scroll did not use the latest line")
config.options.scroll_debounce_ms = 1

local request_error = nil
client.request_raw("GET", "/fail", nil, function(err)
  request_error = err
end)
wait_for(function()
  return request_error ~= nil
end, "curl failure did not surface an error")

config.options.open_browser = true
config.options.browser_cmd = "browser-open"
mdlive.open(bufnr)
local browser_call = nil
wait_for(function()
  for _, call in ipairs(calls) do
    if call.args[1] == "browser-open" then
      browser_call = call
      return true
    end
  end
  return false
end, "browser command was not launched")
assert_true(browser_call.args[#browser_call.args]:match("/view/") ~= nil, "browser URL was not passed")
config.options.open_browser = false
config.options.browser_cmd = nil

local export_path = "/tmp/mdlive-export-test.html"
pcall(vim.fn.delete, export_path)
vim.ui.input = function(_, cb)
  cb(export_path)
end
mdlive.export_html(bufnr)
wait_for(function()
  return vim.fn.filereadable(export_path) == 1
end, "export file was not written")
assert_true(table.concat(vim.fn.readfile(export_path), "\n"):match("<!doctype html>") ~= nil, "export HTML is invalid")

local original_executable = vim.fn.executable
vim.fn.executable = function()
  return 0
end
local missing_browser_error = nil
browser_module.export_pdf("http://127.0.0.1/export", "/tmp/missing.pdf", config.options, function(err)
  missing_browser_error = err
end)
vim.fn.executable = original_executable
assert_true(missing_browser_error ~= nil, "missing PDF browser did not return an error")

local pdf_path = "/tmp/mdlive-export-test.pdf"
config.options.pdf_browser_cmd = "chromium"
config.options.pdf_virtual_time_budget_ms = 1234
vim.ui.input = function(_, cb)
  cb(pdf_path)
end
mdlive.export_pdf(bufnr)
local pdf_call = nil
wait_for(function()
  for _, call in ipairs(calls) do
    if call.args[1] == "chromium" then
      pdf_call = call
      return true
    end
  end
  return false
end, "PDF browser was not launched")
assert_true(vim.tbl_contains(pdf_call.args, "--headless"), "PDF browser was not headless")
assert_true(vim.tbl_contains(pdf_call.args, "--virtual-time-budget=1234"), "PDF virtual time budget was not set")
assert_true(vim.tbl_contains(pdf_call.args, "--print-to-pdf=" .. pdf_path), "PDF output path was not passed")
assert_true(pdf_call.args[#pdf_call.args]:match("/api/export/") ~= nil, "PDF export URL was not passed")

local creates_before_restart = count_session_creates()
delay_session_delete = true
mdlive.restart()
wait_for(function()
  return delayed_delete_exit ~= nil
end, "restart did not send delayed session DELETE")
vim.wait(50)
assert_true(count_session_creates() == creates_before_restart, "restart started before stop completed")

delay_session_delete = false
delayed_delete_exit()
wait_for(function()
  return killed_signal == 15 and count_session_creates() == creates_before_restart + 1
end, "restart did not start after stop completed")

killed_signal = nil
delay_session_delete = true
delayed_delete_exit = nil
local creates_before_blocked_start = count_session_creates()
mdlive.stop()
wait_for(function()
  return delayed_delete_exit ~= nil
end, "stop did not send delayed session DELETE")
local blocked_start_error = nil
mdlive.start(bufnr, function(err)
  blocked_start_error = err
end)
assert_true(blocked_start_error ~= nil, "start during stop did not fail")
assert_true(
  count_session_creates() == creates_before_blocked_start,
  "start during stop created a new session"
)
local delete_call = nil
for _, call in ipairs(calls) do
  if call.args[1] == "curl" and call.args[#call.args]:match("/api/session/.+$") then
    for index, arg in ipairs(call.args) do
      if arg == "--request" and call.args[index + 1] == "DELETE" then
        delete_call = call
      end
    end
  end
end
assert_true(delete_call ~= nil, "session DELETE was not sent before stop")
delay_session_delete = false
delayed_delete_exit()
wait_for(function()
  return killed_signal == 15
end, "server process was not stopped")

local restarted = false
mdlive.start(bufnr, function(err)
  assert_true(err == nil, "restart after stop failed: " .. tostring(err))
  restarted = true
end)
wait_for(function()
  return restarted
end, "start after stop callback did not run")

killed_signal = nil
delay_session_delete = true
delayed_delete_exit = nil
local deletes_before_leave = count_session_deletes()
vim.api.nvim_exec_autocmds("VimLeavePre", {})
assert_true(killed_signal == 15, "VimLeavePre did not stop server synchronously")
vim.wait(50)
assert_true(count_session_deletes() == deletes_before_leave, "VimLeavePre waited on session DELETE before stopping")
delay_session_delete = false

config.options.server_bin = "fail-server"
local failed_start = nil
mdlive.start(bufnr, function(err)
  failed_start = err
end)
wait_for(function()
  return failed_start ~= nil and failed_start:match("failed") ~= nil
end, "server startup failure did not reach callback")

local asset, os_name = install.detect_asset({ sysname = "Linux", machine = "x86_64" })
assert_true(asset == "mdlpv-rs-linux-x86_64.tar.gz", "Linux x86_64 asset was not detected")
assert_true(os_name == "linux", "Linux OS name was not normalized")

asset, os_name = install.detect_asset({ sysname = "Darwin", machine = "arm64" })
assert_true(asset == "mdlpv-rs-macos-aarch64.tar.gz", "macOS arm64 asset was not detected")
assert_true(os_name == "macos", "macOS OS name was not normalized")

asset, os_name = install.detect_asset({ sysname = "Windows_NT", machine = "AMD64" })
assert_true(asset == "mdlpv-rs-windows-x86_64.zip", "Windows AMD64 asset was not detected")
assert_true(os_name == "windows", "Windows OS name was not normalized")
assert_true(install.binary_name("windows") == "mdlpv-rs.exe", "Windows binary name was wrong")
assert_true(install.binary_name("linux") == "mdlpv-rs", "Unix binary name was wrong")

local unsupported_ok = pcall(function()
  install.detect_asset({ sysname = "FreeBSD", machine = "riscv64" })
end)
assert_true(not unsupported_ok, "unsupported platform did not fail")

local checksum = install.checksum_for({
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  mdlpv-rs-linux-x86_64.tar.gz",
  "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *mdlpv-rs-windows-x86_64.zip",
}, "mdlpv-rs-windows-x86_64.zip")
assert_true(checksum == "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "checksum was not parsed")
assert_true(
  install.checksum_for({ "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  other.tar.gz" }, asset) == nil,
  "missing checksum did not return nil"
)

local original_detect_asset = install.detect_asset
local original_executable = vim.fn.executable
local original_system = vim.system
local install_tmp = vim.fn.tempname()
local install_plugin_dir = install_tmp .. "/plugin"
vim.fn.mkdir(install_plugin_dir, "p")
install.detect_asset = function()
  return "mdlpv-rs-linux-x86_64.tar.gz", "linux"
end
vim.fn.executable = function(name)
  if name == "curl" or name == "sha256sum" or name == "tar" then
    return 1
  end
  return original_executable(name)
end
vim.system = function(args)
  if args[1] == "curl" then
    local output_path = args[#args - 1]
    local url = args[#args]
    if url:match("/SHA256SUMS$") then
      vim.fn.writefile({
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  mdlpv-rs-linux-x86_64.tar.gz",
      }, output_path)
    else
      vim.fn.writefile({ "archive" }, output_path)
    end
    return {
      wait = function()
        return { code = 0, stdout = "", stderr = "" }
      end,
    }
  end

  if args[1] == "sha256sum" then
    return {
      wait = function()
        return {
          code = 0,
          stdout = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  " .. args[2] .. "\n",
          stderr = "",
        }
      end,
    }
  end

  if args[1] == "tar" then
    local extract_dir = args[#args]
    vim.fn.mkdir(extract_dir .. "/mdlpv-rs", "p")
    vim.fn.writefile({ "installed binary" }, extract_dir .. "/mdlpv-rs/mdlpv-rs")
    return {
      wait = function()
        return { code = 0, stdout = "", stderr = "" }
      end,
    }
  end

  error("unexpected installer command: " .. table.concat(args, " "))
end

local install_ok, installed_binary = pcall(function()
  return install.install({ dir = install_plugin_dir })
end)
vim.system = original_system
vim.fn.executable = original_executable
install.detect_asset = original_detect_asset
local installed_binary_readable = install_ok and vim.fn.filereadable(installed_binary) or 0
local installed_binary_content = installed_binary_readable == 1 and vim.fn.readfile(installed_binary)[1] or nil
vim.fn.delete(install_tmp, "rf")
assert_true(install_ok, installed_binary)
assert_true(installed_binary == install_plugin_dir .. "/mdlpv-rs", "installer returned wrong binary path")
assert_true(installed_binary_readable == 1, "installer did not copy binary")
assert_true(installed_binary_content == "installed binary", "installer copied wrong binary content")

vim.cmd("quitall!")
