// Wrap tables for responsive design
document.addEventListener("DOMContentLoaded", function() {
    const tables = document.querySelectorAll("table");
    tables.forEach(function(table) {
        if (!table.parentElement.classList.contains("table-wrapper")) {
            const wrapper = document.createElement("div");
            wrapper.className = "table-wrapper";
            table.parentNode.insertBefore(wrapper, table);
            wrapper.appendChild(table);
        }
    });
});
