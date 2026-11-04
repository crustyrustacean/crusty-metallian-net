interface RotationAlbum {
  artist: string;
  album: string;
  cover: string;
  year: number;
  note?: string;
}

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

async function loadRotation(): Promise<void> {
  const widget = document.querySelector<HTMLElement>("#current-rotation");
  if (!widget) {
    return;
  }

  try {
    const albums = await loadJson<RotationAlbum[]>("/static/data/rotation.json");
    const pick = pickRandom(albums);
    if (!pick) {
      widget.textContent = "Currently offline...";
      return;
    }

    widget.innerHTML = `
      <div class="rotation-widget">
        <p class="rotation-label">Currently Spinning</p>
        <div class="rotation-content">
          <img src="${pick.cover}" alt="${pick.artist} - ${pick.album}">
          <div class="rotation-details">
            <p class="rotation-artist">${pick.artist}</p>
            <p class="rotation-album">${pick.album}</p>
            <p class="rotation-year">${pick.year}</p>
            ${pick.note ? `<p class="rotation-note">${pick.note}</p>` : ""}
          </div>
        </div>
      </div>
    `;
  } catch (error) {
    console.error("Failed to load rotation:", error);
    widget.textContent = "Currently offline...";
  }
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
  } catch (error) {
    console.error("Failed to load trivia:", error);
    widget.textContent = "";
  }
}

void Promise.all([loadRotation(), loadTrivia()]);
