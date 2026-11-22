local browser = require("mdlive.browser")
local client = require("mdlive.client")
local config = require("mdlive.config")

local M = {}

local sessions = {}
-- Callbacks waiting for an in-progress session creation, keyed by buffer.
local starting = {}
-- Per-buffer update state; only one update request is in flight at a time so
-- the server receives buffer snapshots in order.
local updates = {}
local timers = {}
local scroll_timers = {}
local stopping = false
local augroup = vim.api.nvim_create_augroup("mdlive", { clear = true })

local function notify_error(message)
  client.notify_error(message)
end

-- Sessions belong to one server launch; drop those left by a server that exited.
local function active_session(bufnr)
  local session = sessions[bufnr]
  if session and session.generation ~= client.generation() then
    sessions[bufnr] = nil
    updates[bufnr] = nil
    return nil
  end
  return session
end

local function buffer_payload(bufnr)
  local name = vim.api.nvim_buf_get_name(bufnr)
  local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  return {
    path = name,
    title = vim.fn.fnamemodify(name, ":t"),
    content = table.concat(lines, "\n"),
  }
end

local function schedule_update(bufnr)
  if not sessions[bufnr] then
    return
  end
  if timers[bufnr] then
    timers[bufnr]:stop()
    timers[bufnr]:close()
  end

  local timer = vim.loop.new_timer()
  timers[bufnr] = timer
  timer:start(config.options.debounce_ms, 0, function()
    timer:stop()
    timer:close()
    timers[bufnr] = nil
    vim.schedule(function()
      M.update(bufnr)
    end)
  end)
end

local function close_scroll_timer(bufnr)
  local state = scroll_timers[bufnr]
  if not state then
    return
  end
  if state.timer then
    state.timer:stop()
    state.timer:close()
  end
  scroll_timers[bufnr] = nil
end

local function schedule_scroll(bufnr)
  if not config.options.scroll_sync or not sessions[bufnr] then
    return
  end

  local interval = config.options.scroll_debounce_ms
  if interval <= 0 then
    M.scroll(bufnr)
    return
  end

  local state = scroll_timers[bufnr]
  if not state then
    state = {
      last_sent = nil,
      pending = false,
      timer = nil,
    }
    scroll_timers[bufnr] = state
  end

  local now = vim.loop.now()
  if not state.last_sent then
    state.last_sent = now
    vim.schedule(function()
      M.scroll(bufnr)
    end)
    return
  end

  state.pending = true
  if state.timer then
    return
  end

  local elapsed = now - state.last_sent
  local wait_ms = math.max(interval - elapsed, 0)
  state.timer = vim.loop.new_timer()
  state.timer:start(wait_ms, 0, function()
    local timer = state.timer
    state.timer = nil
    if timer then
      timer:stop()
      timer:close()
    end
    if not state.pending then
      return
    end
    state.pending = false
    state.last_sent = vim.loop.now()
    vim.schedule(function()
      M.scroll(bufnr)
    end)
  end)
end

local function attach_autocmds(bufnr)
  vim.api.nvim_clear_autocmds({ group = augroup, buffer = bufnr })
  vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI", "BufWritePost" }, {
    group = augroup,
    buffer = bufnr,
    callback = function()
      schedule_update(bufnr)
    end,
  })
  if config.options.scroll_sync then
    vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
      group = augroup,
      buffer = bufnr,
      callback = function()
        schedule_scroll(bufnr)
      end,
    })
  end
  vim.api.nvim_create_autocmd("BufUnload", {
    group = augroup,
    buffer = bufnr,
    callback = function()
      M.close_session(bufnr)
    end,
  })
end

-- `entry` identifies one start attempt; attempts aborted by stop are ignored here
-- so their late callbacks cannot finish or clear a newer attempt.
local function finish_start(bufnr, entry, err, session)
  if starting[bufnr] ~= entry then
    return
  end
  starting[bufnr] = nil
  if err then
    notify_error(err)
  end
  for _, cb in ipairs(entry.callbacks) do
    cb(err, session)
  end
end

local function abort_starts(err)
  local aborted = starting
  starting = {}
  for _, entry in pairs(aborted) do
    for _, cb in ipairs(entry.callbacks) do
      vim.schedule(function()
        cb(err)
      end)
    end
  end
end

function M.start(bufnr, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  if stopping then
    local err = "server is stopping"
    notify_error(err)
    if cb then
      cb(err)
    end
    return
  end

  if starting[bufnr] then
    if cb then
      table.insert(starting[bufnr].callbacks, cb)
    end
    return
  end
  local entry = { callbacks = { cb } }
  starting[bufnr] = entry

  client.ensure_server(function(err)
    if starting[bufnr] ~= entry then
      return
    end
    if err then
      finish_start(bufnr, entry, err)
      return
    end

    -- Reuse the buffer's session unless it belongs to a server that has since exited.
    local existing = sessions[bufnr]
    if existing and existing.generation == client.generation() then
      if config.options.open_browser then
        browser.open(existing.view_url, config.options)
      end
      finish_start(bufnr, entry, nil, existing)
      return
    end
    sessions[bufnr] = nil
    updates[bufnr] = nil

    local generation = client.generation()
    client.request("POST", "/api/session", buffer_payload(bufnr), function(req_err, response)
      if starting[bufnr] ~= entry then
        return
      end
      if req_err then
        finish_start(bufnr, entry, req_err)
        return
      end
      if client.generation() ~= generation then
        finish_start(bufnr, entry, "session start was interrupted")
        return
      end
      if not vim.api.nvim_buf_is_loaded(bufnr) then
        client.request("DELETE", "/api/session/" .. response.session_id, nil, function() end)
        finish_start(bufnr, entry, "buffer was unloaded before the session started")
        return
      end
      sessions[bufnr] = {
        id = response.session_id,
        view_url = response.view_url,
        generation = generation,
      }
      attach_autocmds(bufnr)
      if config.options.open_browser then
        browser.open(response.view_url, config.options)
      end
      finish_start(bufnr, entry, nil, sessions[bufnr])
    end)
  end)
end

function M.update(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local session = active_session(bufnr)
  if not session then
    return
  end
  local state = updates[bufnr]
  if state and state.in_flight then
    state.dirty = true
    return
  end
  state = { in_flight = true, dirty = false }
  updates[bufnr] = state
  client.request("POST", "/api/session/" .. session.id, buffer_payload(bufnr), function(err)
    if err then
      notify_error(err)
    end
    if updates[bufnr] ~= state then
      return
    end
    updates[bufnr] = nil
    if state.dirty then
      M.update(bufnr)
    end
  end)
end

function M.scroll(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local session = active_session(bufnr)
  if not session or not config.options.scroll_sync then
    return
  end
  local line = vim.api.nvim_win_get_cursor(0)[1]
  client.request("POST", "/api/session/" .. session.id .. "/scroll", { line = line }, function(err)
    if err then
      notify_error(err)
    end
  end)
end

function M.open(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local session = active_session(bufnr)
  if not session then
    M.start(bufnr)
    return
  end
  browser.open(session.view_url, config.options)
end

local function default_export_path(bufnr, extension)
  local name = vim.api.nvim_buf_get_name(bufnr)
  if name == "" then
    return vim.fn.getcwd() .. "/mdlive-export." .. extension
  end
  return vim.fn.fnamemodify(name, ":r") .. "." .. extension
end

local function write_export(bufnr, output_path)
  local session = active_session(bufnr)
  if not session then
    notify_error("session is not running")
    return
  end
  client.request_raw("GET", "/api/export/" .. session.id, nil, function(err, html)
    if err then
      notify_error(err)
      return
    end
    local ok, write_err = pcall(vim.fn.writefile, vim.split(html, "\n", { plain = true }), output_path)
    if not ok then
      notify_error(write_err)
      return
    end
    vim.schedule(function()
      vim.notify("mdlive: exported " .. output_path, vim.log.levels.INFO)
    end)
  end)
end

local function export_with_session(bufnr, prompt, extension, write)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  vim.ui.input({
    prompt = prompt,
    default = default_export_path(bufnr, extension),
    completion = "file",
  }, function(output_path)
    if not output_path or output_path == "" then
      return
    end
    if active_session(bufnr) then
      write(bufnr, output_path)
      return
    end
    M.start(bufnr, function(err)
      if err then
        return
      end
      write(bufnr, output_path)
    end)
  end)
end

function M.export_html(bufnr)
  export_with_session(bufnr, "Export HTML: ", "html", write_export)
end

local function write_pdf_export(bufnr, output_path)
  local session = active_session(bufnr)
  if not session then
    notify_error("session is not running")
    return
  end

  local url = client.base_url() .. "/api/export/" .. session.id
  browser.export_pdf(url, output_path, config.options, function(err)
    if err then
      notify_error(err)
      return
    end
    vim.schedule(function()
      vim.notify("mdlive: exported " .. output_path, vim.log.levels.INFO)
    end)
  end)
end

function M.export_pdf(bufnr)
  export_with_session(bufnr, "Export PDF: ", "pdf", write_pdf_export)
end

function M.close_session(bufnr, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local session = sessions[bufnr]
  if not session then
    if cb then
      cb()
    end
    return
  end
  sessions[bufnr] = nil
  updates[bufnr] = nil
  close_scroll_timer(bufnr)
  client.request("DELETE", "/api/session/" .. session.id, nil, function()
    if cb then
      cb()
    end
  end)
end

function M.stop(cb, opts)
  opts = opts or {}
  if stopping and not opts.force then
    if cb then
      cb()
    end
    return
  end
  stopping = true
  -- Fail in-progress starts now, so a start issued after stop (e.g. restart)
  -- launches a new server instead of joining an aborted attempt.
  abort_starts("session start was interrupted by stop")

  local bufnrs = {}
  for bufnr, _ in pairs(sessions) do
    table.insert(bufnrs, bufnr)
  end

  for bufnr, timer in pairs(timers) do
    timer:stop()
    timer:close()
    timers[bufnr] = nil
  end
  for bufnr, _ in pairs(scroll_timers) do
    close_scroll_timer(bufnr)
  end

  if #bufnrs == 0 then
    client.stop_server()
    stopping = false
    if cb then
      cb()
    end
    return
  end

  if opts.force then
    for _, bufnr in ipairs(bufnrs) do
      sessions[bufnr] = nil
      updates[bufnr] = nil
    end
    client.stop_server()
    stopping = false
    if cb then
      cb()
    end
    return
  end

  local pending = #bufnrs
  local stopped = false
  local timeout = vim.loop.new_timer()

  local function stop_server_once()
    if stopped then
      return
    end
    stopped = true
    stopping = false
    if timeout then
      timeout:stop()
      timeout:close()
      timeout = nil
    end
    client.stop_server()
    if cb then
      cb()
    end
  end

  timeout:start(200, 0, function()
    vim.schedule(stop_server_once)
  end)

  local function on_closed()
    pending = pending - 1
    if pending == 0 then
      stop_server_once()
    end
  end

  for _, bufnr in ipairs(bufnrs) do
    M.close_session(bufnr, on_closed)
  end
end

function M.restart()
  local bufnr = vim.api.nvim_get_current_buf()
  M.stop(function()
    M.start(bufnr)
  end)
end

function M.setup(opts)
  config.setup(opts)

  vim.api.nvim_create_user_command("MdLiveStart", function()
    M.start()
  end, {})
  vim.api.nvim_create_user_command("MdLiveStop", function()
    M.stop()
  end, {})
  vim.api.nvim_create_user_command("MdLiveOpen", function()
    M.open()
  end, {})
  vim.api.nvim_create_user_command("MdLiveRestart", function()
    M.restart()
  end, {})
  vim.api.nvim_create_user_command("MdLiveExportHtml", function()
    M.export_html()
  end, {})
  vim.api.nvim_create_user_command("MdLiveExportPdf", function()
    M.export_pdf()
  end, {})

  vim.api.nvim_clear_autocmds({ group = augroup, event = "VimLeavePre" })
  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = augroup,
    callback = function()
      M.stop(nil, { force = true })
    end,
  })
end

return M
