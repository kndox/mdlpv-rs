(function () {
  // Exported preview HTML may contain an element with id="katex"; only use the real library.
  const katex = window.katex;
  if (typeof katex?.render !== "function") {
    return;
  }
  document.querySelectorAll(".math").forEach((node) => {
    katex.render(node.textContent, node, {
      displayMode: node.classList.contains("math-display"),
      throwOnError: false,
    });
  });
})();
