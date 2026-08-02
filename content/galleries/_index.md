+++
title = "Galleries"
+++

<div class="gallery-index" id="gallery-index">
    <p>Loading galleries.</p>
</div>

<script>
(async () => {
    const res = await fetch('https://metallian-photos.fly.dev/api/galleries');
    if (!res.ok) {
        document.getElementById('gallery-index').innerHTML =
            '<p>Could not load galleries right now. Please try again later.</p>';
        return;
    }
    const galleries = await res.json();
    if (galleries.length === 0) {
        document.getElementById('gallery-index').innerHTML =
            '<p>No galleries yet.</p>';
        return;
    }
    document.getElementById('gallery-index').innerHTML = galleries.map(g => `
        <li class="gallery-card">
            <a href="https://metallian-photos.fly.dev/g/${g.slug}">
                ${g.cover_photo_id
                    ? `<img src="https://metallian-photos.fly.dev/api/photos/${g.cover_photo_id}/image" alt="${g.band}" loading="lazy">`
                    : ''}
                <div class="gallery-card-info">
                    <p class="gallery-card-band">${g.band}</p>
                    <p class="gallery-card-count">${g.photo_count} photo${g.photo_count !== 1 ? 's' : ''}</p>
                </div>
            </a>
        </li>
    `).join('');
})();
</script>
