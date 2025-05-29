/**
 * 👁️ Perspt Documentation JavaScript Enhancements
 * Adds interactive features and animations to the documentation
 */

(function() {
    'use strict';

    // Initialize when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initializePersptDocs);
    } else {
        initializePersptDocs();
    }

    function initializePersptDocs() {
        addThemeToggle();
        addCopyToClipboard();
        addSmoothScrolling();
        addSearchEnhancements();
        addAnimations();
        addKeyboardShortcuts();
        addTerminalEffects();
        console.log('👁️ Perspt Documentation Enhanced');
    }

    /**
     * Add theme toggle functionality
     */
    function addThemeToggle() {
        const themeToggle = document.createElement('button');
        themeToggle.innerHTML = '🌙';
        themeToggle.className = 'perspt-theme-toggle';
        themeToggle.title = 'Toggle theme';
        themeToggle.style.cssText = `
            position: fixed;
            top: 20px;
            right: 20px;
            background: var(--perspt-gradient-primary);
            border: none;
            border-radius: 50%;
            width: 50px;
            height: 50px;
            font-size: 20px;
            cursor: pointer;
            z-index: var(--perspt-z-fixed);
            box-shadow: var(--perspt-shadow-lg);
            transition: all var(--perspt-transition-normal);
        `;

        themeToggle.addEventListener('click', function() {
            document.body.classList.toggle('perspt-light-theme');
            themeToggle.innerHTML = document.body.classList.contains('perspt-light-theme') ? '☀️' : '🌙';
        });

        document.body.appendChild(themeToggle);
    }

    /**
     * Add copy to clipboard functionality for code blocks
     */
    function addCopyToClipboard() {
        const codeBlocks = document.querySelectorAll('pre code');
        
        codeBlocks.forEach(function(codeBlock) {
            const pre = codeBlock.parentElement;
            const copyButton = document.createElement('button');
            
            copyButton.innerHTML = '📋 Copy';
            copyButton.className = 'perspt-copy-btn';
            copyButton.style.cssText = `
                position: absolute;
                top: 10px;
                right: 10px;
                background: var(--perspt-bg-medium);
                color: var(--perspt-text-primary);
                border: 1px solid var(--perspt-border);
                border-radius: var(--perspt-radius-md);
                padding: 0.5rem 1rem;
                font-size: 0.8rem;
                cursor: pointer;
                opacity: 0;
                transition: all var(--perspt-transition-normal);
                z-index: 10;
            `;

            copyButton.addEventListener('click', function() {
                navigator.clipboard.writeText(codeBlock.textContent).then(function() {
                    copyButton.innerHTML = '✅ Copied!';
                    copyButton.style.background = 'var(--perspt-success)';
                    
                    setTimeout(function() {
                        copyButton.innerHTML = '📋 Copy';
                        copyButton.style.background = 'var(--perspt-bg-medium)';
                    }, 2000);
                });
            });

            pre.style.position = 'relative';
            pre.appendChild(copyButton);

            // Show copy button on hover
            pre.addEventListener('mouseenter', function() {
                copyButton.style.opacity = '1';
            });

            pre.addEventListener('mouseleave', function() {
                copyButton.style.opacity = '0';
            });
        });
    }

    /**
     * Add smooth scrolling for anchor links
     */
    function addSmoothScrolling() {
        const anchors = document.querySelectorAll('a[href^="#"]');
        
        anchors.forEach(function(anchor) {
            anchor.addEventListener('click', function(e) {
                e.preventDefault();
                const target = document.querySelector(this.getAttribute('href'));
                
                if (target) {
                    target.scrollIntoView({
                        behavior: 'smooth',
                        block: 'start'
                    });
                }
            });
        });
    }

    /**
     * Enhance search functionality
     */
    function addSearchEnhancements() {
        const searchInput = document.querySelector('.search-input');
        
        if (searchInput) {
            // Add search shortcuts and live search
            let searchTimeout;
            
            searchInput.addEventListener('input', function() {
                clearTimeout(searchTimeout);
                searchTimeout = setTimeout(function() {
                    // Add search highlighting or filtering logic here
                    console.log('🔍 Searching for:', searchInput.value);
                }, 300);
            });

            // Add keyboard shortcuts
            searchInput.addEventListener('keydown', function(e) {
                if (e.key === 'Escape') {
                    searchInput.blur();
                    searchInput.value = '';
                }
            });
        }
    }

    /**
     * Add subtle animations and effects
     */
    function addAnimations() {
        // Add intersection observer for fade-in animations
        const observerOptions = {
            threshold: 0.1,
            rootMargin: '0px 0px -50px 0px'
        };

        const observer = new IntersectionObserver(function(entries) {
            entries.forEach(function(entry) {
                if (entry.isIntersecting) {
                    entry.target.classList.add('perspt-fade-in');
                }
            });
        }, observerOptions);

        // Observe all documentation blocks
        const docBlocks = document.querySelectorAll('.docblock, .module-item, .impl, .method');
        docBlocks.forEach(function(block) {
            observer.observe(block);
        });

        // Add CSS for fade-in animation
        const style = document.createElement('style');
        style.textContent = `
            .perspt-fade-in {
                animation: persptFadeIn 0.6s ease-out forwards;
            }
            
            @keyframes persptFadeIn {
                from {
                    opacity: 0;
                    transform: translateY(20px);
                }
                to {
                    opacity: 1;
                    transform: translateY(0);
                }
            }
        `;
        document.head.appendChild(style);
    }

    /**
     * Add keyboard shortcuts for documentation navigation
     */
    function addKeyboardShortcuts() {
        document.addEventListener('keydown', function(e) {
            // Don't trigger shortcuts when typing in input fields
            if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
                return;
            }

            switch(e.key) {
                case 's':
                case '/':
                    e.preventDefault();
                    const searchInput = document.querySelector('.search-input');
                    if (searchInput) {
                        searchInput.focus();
                    }
                    break;
                    
                case 'h':
                    e.preventDefault();
                    showKeyboardHelp();
                    break;
                    
                case 'Escape':
                    hideKeyboardHelp();
                    break;
            }
        });
    }

    /**
     * Add terminal-like effects
     */
    function addTerminalEffects() {
        // Add typing effect to code examples
        const codeExamples = document.querySelectorAll('code.language-bash, code.language-sh');
        
        codeExamples.forEach(function(code) {
            const text = code.textContent;
            code.textContent = '';
            code.style.fontFamily = 'var(--perspt-font-mono)';
            
            // Add terminal prompt
            const prompt = document.createElement('span');
            prompt.textContent = '$ ';
            prompt.style.color = 'var(--perspt-primary)';
            prompt.style.fontWeight = 'bold';
            code.appendChild(prompt);
            
            // Add cursor
            const cursor = document.createElement('span');
            cursor.textContent = '▋';
            cursor.style.color = 'var(--perspt-primary)';
            cursor.style.animation = 'cursor 1s infinite';
            
            let i = 0;
            const typeText = function() {
                if (i < text.length) {
                    const char = text.charAt(i);
                    const span = document.createElement('span');
                    span.textContent = char;
                    code.insertBefore(span, cursor);
                    i++;
                    setTimeout(typeText, Math.random() * 100 + 50);
                } else {
                    cursor.style.display = 'none';
                }
            };
            
            code.appendChild(cursor);
            
            // Start typing when element comes into view
            const observer = new IntersectionObserver(function(entries) {
                entries.forEach(function(entry) {
                    if (entry.isIntersecting) {
                        setTimeout(typeText, 1000);
                        observer.unobserve(entry.target);
                    }
                });
            });
            
            observer.observe(code);
        });
    }

    /**
     * Show keyboard shortcuts help modal
     */
    function showKeyboardHelp() {
        const modal = document.createElement('div');
        modal.className = 'perspt-help-modal';
        modal.innerHTML = `
            <div class="perspt-help-content">
                <h3>⌨️ Keyboard Shortcuts</h3>
                <div class="perspt-help-shortcuts">
                    <div class="perspt-shortcut">
                        <kbd>s</kbd> or <kbd>/</kbd>
                        <span>Focus search</span>
                    </div>
                    <div class="perspt-shortcut">
                        <kbd>h</kbd>
                        <span>Show this help</span>
                    </div>
                    <div class="perspt-shortcut">
                        <kbd>Esc</kbd>
                        <span>Close modals</span>
                    </div>
                    <div class="perspt-shortcut">
                        <kbd>Ctrl</kbd> + <kbd>C</kbd>
                        <span>Copy code (when hovering)</span>
                    </div>
                </div>
                <button class="perspt-help-close">Close</button>
            </div>
        `;
        
        modal.style.cssText = `
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: var(--perspt-bg-overlay);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: var(--perspt-z-modal);
            backdrop-filter: blur(10px);
        `;

        const content = modal.querySelector('.perspt-help-content');
        content.style.cssText = `
            background: var(--perspt-bg-medium);
            border: 2px solid var(--perspt-primary);
            border-radius: var(--perspt-radius-lg);
            padding: 2rem;
            max-width: 400px;
            width: 90%;
            box-shadow: var(--perspt-shadow-xl);
        `;

        const closeBtn = modal.querySelector('.perspt-help-close');
        closeBtn.addEventListener('click', function() {
            hideKeyboardHelp();
        });

        modal.addEventListener('click', function(e) {
            if (e.target === modal) {
                hideKeyboardHelp();
            }
        });

        document.body.appendChild(modal);
        
        // Store reference for cleanup
        window.persptHelpModal = modal;
    }

    /**
     * Hide keyboard shortcuts help modal
     */
    function hideKeyboardHelp() {
        if (window.persptHelpModal) {
            document.body.removeChild(window.persptHelpModal);
            window.persptHelpModal = null;
        }
    }

})();
