"""Generate the schematic search-trajectory figure for PSP-10."""

from pathlib import Path

import matplotlib.pyplot as plt


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "source" / "psp-000010" / "search-trajectories.svg"


def main() -> None:
    """Render a deterministic, publication-sized SVG."""
    plt.rcParams.update(
        {
            "font.family": "DejaVu Sans",
            "font.size": 9,
            "axes.labelsize": 9,
            "axes.titlesize": 10,
            "legend.fontsize": 8,
            "svg.fonttype": "none",
        }
    )

    actions = [0, 1, 2, 3, 4]
    branch_a = [8.0, 9.4, 10.1, 7.5, 6.5]
    branch_b = [8.0, 8.3, 7.9, 7.7, 7.6]
    branch_c = [8.0, 9.0, 9.6, 9.2, 8.8]
    threshold = 7.0

    fig, ax = plt.subplots(figsize=(7.2, 3.6), constrained_layout=True)
    ax.axhspan(0, threshold, color="#DDEFE4", alpha=0.75, zorder=0)
    ax.axhline(
        threshold,
        color="#347A55",
        linewidth=1.2,
        linestyle="--",
        label=r"descent threshold $V_{best}-\rho$",
    )
    ax.plot(actions, branch_a, marker="o", color="#3B6FB6", label="branch A: eligible")
    ax.plot(actions, branch_b, marker="s", color="#A06A1B", label="branch B: ineligible")
    ax.plot(actions, branch_c, marker="^", color="#8A5968", label="branch C: abandoned")

    ax.scatter([0], [8.0], s=70, facecolor="white", edgecolor="#222222", zorder=5)
    ax.annotate(
        r"accepted root $x_k$",
        xy=(0, 8.0),
        xytext=(0.25, 10.6),
        arrowprops={"arrowstyle": "->", "color": "#444444", "lw": 0.9},
    )
    ax.scatter([4], [6.5], s=80, facecolor="#DDEFE4", edgecolor="#347A55", zorder=5)
    ax.annotate(
        r"one commit: $x_{k+1}$",
        xy=(4, 6.5),
        xytext=(2.55, 5.25),
        arrowprops={"arrowstyle": "->", "color": "#347A55", "lw": 0.9},
    )

    ax.set_title("Speculative paths may rise; the accepted trajectory may not")
    ax.set_xlabel("branch action")
    ax.set_ylabel(r"energy $V$ (schematic units)")
    ax.set_xlim(-0.15, 4.15)
    ax.set_ylim(4.8, 11.0)
    ax.set_xticks(actions)
    ax.spines[["top", "right"]].set_visible(False)
    ax.grid(axis="y", color="#D7DCE2", linewidth=0.7)
    ax.legend(loc="upper right", frameon=False)

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUTPUT, format="svg", metadata={"Date": None})
    plt.close(fig)


if __name__ == "__main__":
    main()
