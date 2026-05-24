local config = require("mdlive.config")

local M = {}

M.state = {
  process = nil,
  host = nil,
  port = nil,
  starting = false,
  waiters = {},
}

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

local function base_url()
  return string.format("http://%s:%s", M.state.host, M.state.port)
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
    opts.mermaid.mode,
    "--mermaid-cdn-url",
    opts.mermaid.cdn_url,
  }

  local stdout = ""
  local stderr = ""
  M.state.process = vim.system(args, {
    text = true,
    stdout = function(_, data)
      if not data or M.state.host then
        return
      end
      stdout = stdout .. data
      local line = stdout:match("([^\r\n]+)")
      if not line then
        return
      end
      local ok, decoded = pcall(vim.json.decode, line)
      if ok and decoded.host and decoded.port then
        M.state.host = decoded.host
        M.state.port = decoded.port
        M.state.starting = false
        flush_waiters(nil)
      end
    end,
    stderr = function(_, data)
      if data and data:match("%S") then
        if not M.state.host then
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
    local was_ready = M.state.host ~= nil
    M.state.process = nil
    M.state.host = nil
    M.state.port = nil
    M.state.starting = false
    if not was_ready then
      local message = "server exited before startup: " .. tostring(result.code)
      local trimmed_stderr = vim.trim(stderr)
      if trimmed_stderr ~= "" then
        message = message .. ": " .. trimmed_stderr
      end
      flush_waiters(message)
    end
  end)
end

function M.stop_server()
  if M.state.process then
    pcall(function()
      M.state.process:kill(15)
    end)
  end
  M.state.process = nil
  M.state.host = nil
  M.state.port = nil
  M.state.starting = false
  M.state.waiters = {}
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
    "--request",
    method,
    "--header",
    "Content-Type: application/json",
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
