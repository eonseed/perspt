import matplotlib.pyplot as plt
import numpy as np

# Set up the style for a minimalist, theme-independent design
plt.rcParams['figure.facecolor'] = 'none'
plt.rcParams['axes.facecolor'] = 'none'
plt.rcParams['savefig.facecolor'] = 'none'

fig, ax = plt.subplots(figsize=(8.5, 4.0), dpi=150)

# Configure grid and axes
ax.set_xlim(0, 30)
ax.set_ylim(0, 15)
ax.set_aspect('equal')

# Use a medium neutral color for grid/spines to work on both dark/light themes
neutral_color = '#64748b'  # Slate-500
ax.grid(True, color=neutral_color, alpha=0.15, linestyle=':', linewidth=0.8)

# Remove outer spines for a clean floating look
for spine in ax.spines.values():
    spine.set_visible(False)

# Remove tick marks and labels to keep it purely diagrammatic
ax.set_xticks([])
ax.set_yticks([])

# ----------------- SECTION 1: Decentralized ABM (Left) -----------------
# Generate a swarm of agents with simple local vector headings
np.random.seed(42)
num_swarm = 14
x_swarm = np.random.uniform(2, 8, num_swarm)
y_swarm = np.random.uniform(3, 12, num_swarm)

# Draw swarm agents
ax.scatter(x_swarm, y_swarm, color='#3b82f6', s=35, alpha=0.8, zorder=3, label='Agent')

# Draw local interaction vectors (decentralized rules)
for i in range(num_swarm):
    # Determine a heading pointing loosely toward neighbors
    angle = np.random.uniform(0, 2 * np.pi)
    dx = 0.6 * np.cos(angle)
    dy = 0.6 * np.sin(angle)
    ax.arrow(x_swarm[i], y_swarm[i], dx, dy, head_width=0.18, head_length=0.25,
             fc='#3b82f6', ec='#3b82f6', alpha=0.5, length_includes_head=True, zorder=2)
    
    # Draw dotted lines to nearby neighbors to show interaction topology
    for j in range(i + 1, num_swarm):
        dist = np.hypot(x_swarm[i] - x_swarm[j], y_swarm[i] - y_swarm[j])
        if dist < 2.5:
            ax.plot([x_swarm[i], x_swarm[j]], [y_swarm[i], y_swarm[j]], 
                    color='#3b82f6', alpha=0.15, linestyle='--', linewidth=0.8, zorder=1)

# ----------------- SECTION 2: Local Rules / Glider (Center) -----------------
# Draw a Game of Life glider on a local cell subgrid
glider_origin_x, glider_origin_y = 12, 5
# A glider pattern relative to its origin
# . X .
# . . X
# X X X
glider_cells = np.array([
    [1, 2],
    [2, 1],
    [2, 0],
    [1, 0],
    [0, 0]
])

# Draw local cell grid lines in the center
subgrid_size = 5
for x in range(subgrid_size + 1):
    ax.plot([glider_origin_x + x, glider_origin_x + x], [glider_origin_y, glider_origin_y + subgrid_size],
            color=neutral_color, alpha=0.25, linestyle='-', linewidth=0.6, zorder=1)
for y in range(subgrid_size + 1):
    ax.plot([glider_origin_x, glider_origin_x + subgrid_size], [glider_origin_y + y, glider_origin_y + y],
            color=neutral_color, alpha=0.25, linestyle='-', linewidth=0.6, zorder=1)

# Fill the active glider cells
for cell in glider_cells:
    rect = plt.Rectangle((glider_origin_x + cell[0], glider_origin_y + cell[1]), 1, 1,
                         facecolor='#f59e0b', alpha=0.85, edgecolor='#d97706', linewidth=0.8, zorder=3)
    ax.add_patch(rect)

# Draw motion trajectory arrow for the glider
ax.annotate('', xy=(glider_origin_x + 4.5, glider_origin_y + 4.5),
            xytext=(glider_origin_x + 1.5, glider_origin_y + 1.5),
            arrowprops=dict(arrowstyle="->", color='#f59e0b', lw=1.5, ls='--', alpha=0.8,
                            shrinkA=0, shrinkB=0), zorder=4)

# ----------------- SECTION 3: Lyapunov Attractor (Right) -----------------
# Generate a spiral vector field representing convergence to a stable manifold
center_x, center_y = 24.5, 7.5

# Draw a spiral path (attractor trajectory)
theta = np.linspace(0, 4 * np.pi, 200)
r = np.linspace(5.0, 0.2, 200)
x_spiral = center_x + r * np.cos(theta)
y_spiral = center_y + r * np.sin(theta)
ax.plot(x_spiral, y_spiral, color='#10b981', alpha=0.35, linestyle='-', linewidth=1.5, zorder=1)

# Place agent nodes along the convergence trajectory with arrows pointing toward the center
spiral_agents_theta = np.array([0.2, 1.2, 2.5, 4.5, 6.8, 9.5])
spiral_agents_r = np.linspace(4.5, 0.8, len(spiral_agents_theta))
x_agents = center_x + spiral_agents_r * np.cos(spiral_agents_theta)
y_agents = center_y + spiral_agents_r * np.sin(spiral_agents_theta)

ax.scatter(x_agents, y_agents, color='#10b981', s=35, alpha=0.9, zorder=3)

# Draw vector arrows toward the sink
for i in range(len(x_agents)):
    # Velocity vector components pointing inward and slightly tangential
    tx = -(x_agents[i] - center_x) - 0.5 * (y_agents[i] - center_y)
    ty = -(y_agents[i] - center_y) + 0.5 * (x_agents[i] - center_x)
    norm = np.hypot(tx, ty)
    dx = (tx / norm) * 0.8
    dy = (ty / norm) * 0.8
    ax.arrow(x_agents[i], y_agents[i], dx, dy, head_width=0.18, head_length=0.25,
             fc='#10b981', ec='#10b981', alpha=0.7, length_includes_head=True, zorder=2)

# Draw the stable attractor basin (central star/manifold)
ax.scatter([center_x], [center_y], color='#10b981', marker='*', s=140, 
           edgecolor='#059669', linewidth=1.0, zorder=4, label='Attractor')

# Draw a dashed circle around the attractor showing the convergence boundary
boundary_circle = plt.Circle((center_x, center_y), 1.5, color='#10b981', 
                             fill=False, linestyle='--', alpha=0.3, linewidth=0.8, zorder=2)
ax.add_patch(boundary_circle)

# ----------------- Annotations & Section Labels -----------------
# Section Labels at the bottom
label_y = 1.0
ax.text(5.0, label_y, "Decentralized SWARM\n(Agent-Based Model)", color=neutral_color,
        fontsize=9, ha='center', va='top', fontweight='normal', alpha=0.85)
ax.text(14.5, label_y, "Local State Rules\n(Game of Life Glider)", color=neutral_color,
        fontsize=9, ha='center', va='top', fontweight='normal', alpha=0.85)
ax.text(24.5, label_y, "Lyapunov Attractor\n(Stable Manifold Convergence)", color=neutral_color,
        fontsize=9, ha='center', va='top', fontweight='normal', alpha=0.85)

plt.tight_layout()
plt.show()
