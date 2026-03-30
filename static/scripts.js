// Site scripts

'use strict';

// Current Rotation Widget
function loadRotation() {
    const widget = document.getElementById('current-rotation');
    
    // only run on pages that have the widget
    if (!widget) return;

    fetch('/static/data/rotation.json')
        .then(r => r.json())
        .then(albums => {
            const pick = albums[Math.floor(Math.random() * albums.length)];
            
            widget.innerHTML = `
                <div class="rotation-widget">
                    <p class="rotation-label">Currently Spinning</p>
                    <div class="rotation-content">
                        <img src="${pick.cover}" alt="${pick.artist} - ${pick.album}">
                        <div class="rotation-details">
                            <p class="rotation-artist">${pick.artist}</p>
                            <p class="rotation-album">${pick.album}</p>
                            <p class="rotation-year">${pick.year}</p>
                            ${pick.note ? `<p class="rotation-note">${pick.note}</p>` : ''}
                        </div>
                    </div>
                </div>
            `;
        })
        .catch(err => {
            console.error('Failed to load rotation:', err);
            widget.innerHTML = '<p>Currently offline...</p>';
        });
}

// Trivia Widget
function loadTrivia() {
    const widget = document.getElementById('metal-trivia');
    
    // only run on pages that have the widget
    if (!widget) return;

    fetch('/static/data/trivia.json')
        .then(r => r.json())
        .then(trivia => {
            const pick = trivia[Math.floor(Math.random() * trivia.length)];
            
            widget.innerHTML = `
                <div class="trivia-widget">
                    <h3>Metal Trivia</h3>
                    <p class="trivia-fact">${pick.fact}</p>
                </div>
            `;
        })
        .catch(err => {
            console.error('Failed to load trivia:', err);
            widget.innerHTML = '';
        });
}

document.addEventListener('DOMContentLoaded', () => {
    loadRotation();
    loadTrivia();
});