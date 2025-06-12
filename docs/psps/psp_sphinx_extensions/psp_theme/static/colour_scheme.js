// CSS-based theme toggle with session persistence
document.addEventListener("DOMContentLoaded", function() {
    const themeToggle = document.querySelector('.theme-toggle-input');
    if (themeToggle) {
        // Load saved theme preference on page load
        const savedTheme = localStorage.getItem('psp-theme');
        
        // Set initial state based on saved preference or system preference
        if (savedTheme === 'dark') {
            themeToggle.checked = true;
        } else if (savedTheme === 'light') {
            themeToggle.checked = false;
        } else {
            // No saved preference, use system preference
            const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            themeToggle.checked = prefersDark;
            // Save the initial preference
            localStorage.setItem('psp-theme', prefersDark ? 'dark' : 'light');
        }
        
        // Remove the initial class now that checkbox state is set
        document.documentElement.classList.remove('theme-dark-initial');
        
        // Save theme preference when changed
        themeToggle.addEventListener('change', function() {
            const newTheme = this.checked ? 'dark' : 'light';
            localStorage.setItem('psp-theme', newTheme);
            console.log('Theme saved:', newTheme); // Debug log
        });
        
        // Also listen for system theme changes and update if no explicit preference
        window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function(e) {
            // Only auto-update if user hasn't explicitly set a preference recently
            const savedTheme = localStorage.getItem('psp-theme');
            if (!savedTheme) {
                themeToggle.checked = e.matches;
            }
        });
    }
});
