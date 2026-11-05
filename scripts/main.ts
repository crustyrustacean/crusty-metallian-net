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

// The current-rotation widget is served by Datastar from the music feed
// service (see `data-init` in templates/home.html), so there is nothing to
// do for it here.

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

void loadTrivia();
