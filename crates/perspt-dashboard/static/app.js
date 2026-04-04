// Perspt Dashboard — minimal client-side helpers

var LIGHT_THEME = 'perspt-light';
var DARK_THEME = 'perspt-dark';

function isDark() {
    return document.documentElement.getAttribute('data-theme') === DARK_THEME;
}

function updateThemeIcon() {
    var sun = document.getElementById('theme-icon-sun');
    var moon = document.getElementById('theme-icon-moon');
    if (!sun || !moon) return;
    // Dark mode: show sun (click to go light). Light mode: show moon (click to go dark).
    sun.style.display = isDark() ? '' : 'none';
    moon.style.display = isDark() ? 'none' : '';
}

// Theme persistence — restore on load, migrate stale values
(function() {
    var saved = localStorage.getItem('perspt-theme');
    if (saved) {
        if (saved === 'dark') saved = DARK_THEME;
        if (saved === 'light') saved = LIGHT_THEME;
        document.documentElement.setAttribute('data-theme', saved);
        localStorage.setItem('perspt-theme', saved);
    }
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', updateThemeIcon);
    } else {
        updateThemeIcon();
    }
})();

function toggleTheme() {
    var next = isDark() ? LIGHT_THEME : DARK_THEME;
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('perspt-theme', next);
    updateThemeIcon();
}

// Clipboard copy helper
function copyToClipboard(text) {
    navigator.clipboard.writeText(text);
}
