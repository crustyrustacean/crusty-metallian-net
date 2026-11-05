interface TriviaFact {
  fact: string;
}

async function loadJson<T>(path: string): Promise<T> {
  const response = await fetch(path);
  if (!response.ok) {
    throw new Error(`${path} returned ${response.status}`);
  }
  return response.json() as Promise<T>;
}

function pickRandom<T>(items: T[]): T | undefined {
  return items[Math.floor(Math.random() * items.length)];
}

// The current-rotation widget is streamed in by Datastar from the music feed
// service (see `data-init` in templates/home.html). The service patches with
// `mode append`, so the widget lands *after* the "Loading current rotation…"
// placeholder rather than replacing it, and nothing clears `aria-busy`.
// Watch for the patch and tidy up once the widget is in the DOM.
function settleRotationWidget(widget: HTMLElement): boolean {
  const widgets = widget.querySelectorAll(".rotation-widget");
  if (widgets.length === 0) {
    return false;
  }

  // A reconnecting stream would append a second copy; keep the newest.
  widgets.forEach((stale, index) => {
    if (index < widgets.length - 1) {
      stale.remove();
    }
  });

  widget.querySelector(".widget-loading")?.remove();
  widget.removeAttribute("aria-busy");
  return true;
}

function showRotationOffline(widget: HTMLElement): void {
  // The widget beat us to it; leave the real content alone.
  if (widget.querySelector(".rotation-widget")) {
    return;
  }
  const loading = widget.querySelector(".widget-loading");
  if (loading) {
    loading.textContent = "Rotation is offline right now.";
  }
  widget.removeAttribute("aria-busy");
}

function watchRotation(): void {
  const widget = document.querySelector<HTMLElement>("#rotation-list");
  if (!widget) {
    return;
  }

  // The patch may have landed before this script ran.
  settleRotationWidget(widget);

  // Keep watching rather than disconnecting after the first patch: the SSE
  // stream reconnects and re-appends, and each append would otherwise stack
  // another copy of the widget. `settleRotationWidget` is idempotent, so
  // repeat calls just confirm there is nothing left to do.
  new MutationObserver(() => {
    settleRotationWidget(widget);
  }).observe(widget, { childList: true, subtree: false });

  // If the feed never answers, the placeholder would otherwise sit on
  // "Loading current rotation…" indefinitely. Datastar retries ten times
  // with exponential backoff capped at 30s before it reports
  // `retries-failed`, so that event alone is too slow to be useful; this
  // listener still catches the fast failures.
  document.addEventListener("datastar-fetch", (event) => {
    const detail = (event as CustomEvent<{ type: string; el?: Element }>).detail;
    if (detail?.el !== widget) {
      return;
    }
    if (detail.type === "error" || detail.type === "retries-failed") {
      showRotationOffline(widget);
    }
  });

  // Give the stream a reasonable window, then stop promising a result.
  setTimeout(() => {
    showRotationOffline(widget);
  }, 10_000);
}

async function loadTrivia(): Promise<void> {
  const widget = document.querySelector<HTMLElement>("#metal-trivia");
  if (!widget) {
    return;
  }

  try {
    const trivia = await loadJson<TriviaFact[]>("/static/data/trivia.json");
    const pick = pickRandom(trivia);
    if (!pick) {
      widget.textContent = "";
      return;
    }

    widget.innerHTML = `
      <div class="trivia-widget">
        <h3>Metal Trivia</h3>
        <p class="trivia-fact">${pick.fact}</p>
      </div>
    `;
    widget.removeAttribute("aria-busy");
  } catch (error) {
    console.error("Failed to load trivia:", error);
    widget.textContent = "";
  }
}

watchRotation();
void loadTrivia();
