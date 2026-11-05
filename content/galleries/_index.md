+++
title = "Galleries"
description = "Photo galleries from select concerts past."
+++

## Photo Galleries

Here be links to galleries from select concerts past.

<ul class="gallery-index" id="gallery-index">
    <li>Loading galleries.</li>
</ul>

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
    // The first row sits above the fold, so its covers are the LCP
    // candidates: load them eagerly with high priority, defer the rest.
    const aboveFold = Math.max(1, Math.floor(
        document.getElementById('gallery-index').getBoundingClientRect().width / 320
    ) * 2);
    document.getElementById('gallery-index').innerHTML = galleries.map((g, i) => `
        <li class="gallery-card">
            <a href="https://metallian-photos.fly.dev/g/${g.slug}">
                ${g.cover_photo_id
                    // The API serves the full-size photo with no resize
                    // parameter, so request it eagerly at a fixed size and
                    // cap it in CSS rather than blowing the page weight.
                    ? `<img src="https://metallian-photos.fly.dev/api/photos/${g.cover_photo_id}/image" alt="${g.band}" width="640" height="480"${i < aboveFold ? ' fetchpriority="high"' : ' loading="lazy"'} decoding="async">`
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
