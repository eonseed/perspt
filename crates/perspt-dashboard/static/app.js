// Perspt Dashboard — minimal client-side helpers

// Theme persistence
(function() {
    var saved = localStorage.getItem('perspt-theme');
    if (saved) {
        document.documentElement.setAttribute('data-theme', saved);
    }
})();

function toggleTheme() {
    var current = document.documentElement.getAttribute('data-theme');
    var next = current === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('perspt-theme', next);
}

// Clipboard copy helper
function copyToClipboard(text) {
    navigator.clipboard.writeText(text);
}
