if vim.g.loaded_mdlive == 1 then
  return
end
vim.g.loaded_mdlive = 1

require("mdlive").setup()
