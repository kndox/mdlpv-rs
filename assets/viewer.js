(function () {
  const content = document.getElementById("content");
  const status = document.getElementById("status");
  const config = window.MDLIVE_CONFIG;
  let currentRevision = -1;
  let mermaidLoaded = false;
  let katexLoaded = false;
  let lastScrollLine = null;
  let pendingScrollLine = null;
  let pendingScrollBehavior = "smooth";
  let scrollFrame = null;
  let eventSource = null;

  async function refresh() {
    const res = await fetch(`/api/rendered/${config.sessionId}`);
    if (!res.ok) {
      throw new Error(`fetch failed: ${res.status}`);
    }

    const data = await res.json();
    if (data.revision <= currentRevision) {
      return;
    }

    currentRevision = data.revision;
    document.title = data.title || "Markdown Live Preview";
    content.innerHTML = data.html;

    await renderMermaidIfNeeded(data.has_mermaid);
    await renderMathIfNeeded(data.has_math);
    if (lastScrollLine !== null) {
      queueScrollToSourceLine(lastScrollLine, "smooth");
    }
    status.textContent = `revision ${data.revision}`;
  }

  function connectSse() {
    eventSource = new EventSource(`/events/${config.sessionId}`);
    eventSource.addEventListener("update", () => refresh().catch(showError));
    eventSource.addEventListener("scroll", (event) => {
      try {
        const data = JSON.parse(event.data);
        if (Number.isFinite(data.line)) {
          lastScrollLine = data.line;
          queueScrollToSourceLine(data.line, "smooth");
        }
      } catch (err) {
        showError(err);
      }
    });
    eventSource.addEventListener("close", closeViewer);
    eventSource.onerror = () => {
      status.textContent = "reconnecting...";
    };
  }

  function closeViewer() {
    if (eventSource) {
      eventSource.close();
      eventSource = null;
    }
    status.textContent = "session stopped";
    window.close();
    setTimeout(() => {
      status.textContent = "session stopped";
    }, 100);
  }

  async function renderMermaidIfNeeded(hasMermaid) {
    if (!hasMermaid) {
      return;
    }

    normalizeMermaidBlocks(content);
    const mermaid = await ensureMermaid();
    if (!mermaid) {
      status.textContent = "Mermaid disabled";
      return;
    }

    await mermaid.run({ querySelector: ".mermaid" });
  }

  async function renderMathIfNeeded(hasMath) {
    if (!hasMath) {
      return;
    }

    const katex = await ensureKatex();
    const nodes = content.querySelectorAll(".math");
    for (const node of nodes) {
      katex.render(node.textContent, node, {
        displayMode: node.classList.contains("math-display"),
        throwOnError: false,
      });
    }
  }

  function normalizeMermaidBlocks(root) {
    const blocks = root.querySelectorAll("pre > code.language-mermaid");
    for (const code of blocks) {
      const pre = code.parentElement;
      const div = document.createElement("div");
      div.className = "mermaid";
      div.textContent = code.textContent;
      pre.replaceWith(div);
    }
  }

  function queueScrollToSourceLine(line, behavior) {
    pendingScrollLine = line;
    pendingScrollBehavior = behavior;
    if (scrollFrame !== null) {
      return;
    }
    scrollFrame = requestAnimationFrame(() => {
      const line = pendingScrollLine;
      const behavior = pendingScrollBehavior;
      scrollFrame = null;
      pendingScrollLine = null;
      scrollToSourceLine(line, behavior);
    });
  }

  function scrollToSourceLine(line, behavior) {
    const anchors = content.querySelectorAll(".mdlive-source-anchor[data-source-line]");
    let target = null;
    for (const anchor of anchors) {
      const anchorLine = Number(anchor.dataset.sourceLine);
      if (!Number.isFinite(anchorLine) || anchorLine > line) {
        break;
      }
      target = anchor;
    }
    if (!target && anchors.length > 0) {
      target = anchors[0];
    }
    if (target) {
      target.scrollIntoView({ block: "start", behavior });
    }
  }

  async function ensureMermaid() {
    if (mermaidLoaded || window.mermaid) {
      mermaidLoaded = true;
      return window.mermaid;
    }
    if (config.mermaidMode === "none") {
      return null;
    }

    const src = config.mermaidMode === "cdn"
      ? config.mermaidCdnUrl
      : "/assets/mermaid/mermaid.min.js";

    try {
      await loadScript(src);
    } catch (err) {
      if (config.mermaidMode === "local-with-cdn-fallback") {
        await loadScript(config.mermaidCdnUrl);
      } else {
        throw new Error("Mermaid asset not found");
      }
    }

    if (!window.mermaid) {
      throw new Error("Mermaid failed to load");
    }

    window.mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
    });
    mermaidLoaded = true;
    return window.mermaid;
  }

  async function ensureKatex() {
    if (katexLoaded || window.katex) {
      katexLoaded = true;
      return window.katex;
    }

    await loadStylesheet("/assets/katex/katex.min.css");
    await loadScript("/assets/katex/katex.min.js");

    if (!window.katex) {
      throw new Error("KaTeX failed to load");
    }

    katexLoaded = true;
    return window.katex;
  }

  function loadScript(src) {
    return new Promise((resolve, reject) => {
      const existing = document.querySelector(`script[data-mdlive-mermaid="${src}"]`);
      if (existing) {
        if (existing.dataset.mdliveLoaded === "true") {
          resolve();
          return;
        }
        existing.addEventListener("load", resolve, { once: true });
        existing.addEventListener("error", reject, { once: true });
        return;
      }

      const script = document.createElement("script");
      script.src = src;
      script.async = true;
      script.dataset.mdliveMermaid = src;
      script.onload = () => {
        script.dataset.mdliveLoaded = "true";
        resolve();
      };
      script.onerror = reject;
      document.head.appendChild(script);
    });
  }

  function loadStylesheet(href) {
    return new Promise((resolve, reject) => {
      const existing = document.querySelector(`link[data-mdlive-style="${href}"]`);
      if (existing) {
        if (existing.dataset.mdliveLoaded === "true") {
          resolve();
          return;
        }
        existing.addEventListener("load", resolve, { once: true });
        existing.addEventListener("error", reject, { once: true });
        return;
      }

      const link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = href;
      link.dataset.mdliveStyle = href;
      link.onload = () => {
        link.dataset.mdliveLoaded = "true";
        resolve();
      };
      link.onerror = reject;
      document.head.appendChild(link);
    });
  }

  function showError(err) {
    console.error(err);
    status.textContent = String(err.message || err);
  }

  refresh().catch(showError);
  connectSse();
})();
