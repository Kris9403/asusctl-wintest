"""
Generates a labeled top-down diagram of the G615LR's 16 Aura zones, so a
human can point at exactly which physical zone lit up during a test using
unambiguous names instead of "back right"/"back left" (which depend on
which way you're facing the laptop and have already caused confusion once).

Orientation: viewed from above, laptop open, standing in front of it.
Top of image = back edge (hinge/screen side). Bottom = front edge (nearest
the user). Matches the convention used throughout HANDOFF.md /
aura_core.ps1's $PHYSICAL_ZONES.

CORRECTED 2026-07-23 against usb_capture_session3/ground_truth/WDL_G615LR.csv
(ASUS's own official Aura Creator device profile for this exact laptop) --
the first version of this diagram had the back edge (0x04-0x07) and the
left sidebar's front/back split (0x09/0x0B) wrong. See aura_core.ps1's
header comment and HANDOFF.md "Windows session 3" for the full story.

Each label shows the physical name (what a human says) and the wire zone
ID (what's actually in the 0x04 packet's zone-ID bytes) -- so a report
like "zone X lit up" can be matched straight back to a specific hex byte.
"""
import matplotlib.pyplot as plt
from matplotlib.patches import FancyBboxPatch, Circle
from matplotlib.lines import Line2D

fig, ax = plt.subplots(figsize=(11, 9), dpi=150)
ax.set_xlim(0, 10)
ax.set_ylim(0, 10)
ax.axis("off")
ax.set_title(
    "ROG Strix G16 2025 (G615LR) — Aura zone map (corrected against ASUS's own Aura Creator device profile)\n"
    "(top-down, laptop open, viewed standing in front of it — back/hinge edge at top)",
    fontsize=12, fontweight="bold", pad=14
)

# Laptop deck outline
deck = FancyBboxPatch((1.5, 1.0), 7.0, 7.5, boxstyle="round,pad=0,rounding_size=0.3",
                       linewidth=2, edgecolor="black", facecolor="#dddddd", zorder=1)
ax.add_patch(deck)

# Keyboard deck sub-rectangle (visual only, roughly where the keys are)
kbd_area = FancyBboxPatch((2.3, 4.6), 5.4, 3.0, boxstyle="round,pad=0,rounding_size=0.1",
                           linewidth=1, edgecolor="#888888", facecolor="#eeeeee", zorder=2)
ax.add_patch(kbd_area)

ax.text(5.0, 8.15, "BACK edge (hinge / screen side)", ha="center", fontsize=9, style="italic", color="#555555")
ax.text(5.0, 0.55, "FRONT edge (nearest user)", ha="center", fontsize=9, style="italic", color="#555555")
ax.text(0.9, 4.75, "LEFT", ha="center", fontsize=9, style="italic", color="#555555", rotation=90)
ax.text(9.1, 4.75, "RIGHT", ha="center", fontsize=9, style="italic", color="#555555", rotation=270)

def zone(x, y, physical, wire, color="#3b82f6", ha="center", va="center", dx=0, dy=0):
    ax.add_patch(Circle((x, y), 0.16, facecolor=color, edgecolor="black", linewidth=1.2, zorder=5))
    label = f"{physical}\n0x{wire:02X}"
    ax.annotate(label, (x, y), xytext=(x + dx, y + dy), ha=ha, va=va,
                fontsize=8.6, fontweight="bold",
                bbox=dict(boxstyle="round,pad=0.25", facecolor="white", edgecolor=color, linewidth=1),
                arrowprops=dict(arrowstyle="-", color=color, linewidth=1) if (dx or dy) else None,
                zorder=6)

LIGHTBAR = "#3b82f6"   # blue = chassis lightbar zones (0x04-0x0F)
KEYBOARD = "#f59e0b"   # amber = keyboard zones (0x00-0x03)

# --- Back edge: corners + bar (corrected -- was flipped in every prior version) ---
zone(1.9, 8.3, "back_corner_left", 0x07, LIGHTBAR, ha="right", dx=-0.3, dy=0.35)
zone(3.7, 8.5, "back_left", 0x05, LIGHTBAR, dy=0.5)
zone(6.3, 8.5, "back_right", 0x04, LIGHTBAR, dy=0.5)
zone(8.1, 8.3, "back_corner_right", 0x06, LIGHTBAR, ha="left", dx=0.3, dy=0.35)

# --- Front edge: corners + bar ---
zone(1.9, 1.2, "front_corner_left", 0x0D, LIGHTBAR, ha="right", dx=-0.3, dy=-0.35)
zone(3.7, 1.0, "front_left", 0x0F, LIGHTBAR, dy=-0.5)
zone(6.3, 1.0, "front_right", 0x0E, LIGHTBAR, dy=-0.5)
zone(8.1, 1.2, "front_corner_right", 0x0C, LIGHTBAR, ha="left", dx=0.3, dy=-0.35)

# --- Left side: front half + back half (corrected -- was swapped) ---
zone(1.55, 6.3, "left_bar_back", 0x09, LIGHTBAR, ha="right", dx=-0.5, dy=0)
zone(1.55, 3.0, "left_bar_front", 0x0B, LIGHTBAR, ha="right", dx=-0.5, dy=0)

# --- Right side: front half + back half ---
zone(8.45, 6.3, "right_bar_back", 0x08, LIGHTBAR, ha="left", dx=0.5, dy=0)
zone(8.45, 3.0, "right_bar_front", 0x0A, LIGHTBAR, ha="left", dx=0.5, dy=0)

# --- Keyboard zones, left to right ---
zone(3.15, 6.1, "kbd1", 0x00, KEYBOARD, dy=0.55)
zone(4.35, 6.1, "kbd2", 0x01, KEYBOARD, dy=0.55)
zone(5.65, 6.1, "kbd3", 0x02, KEYBOARD, dy=0.55)
zone(6.85, 6.1, "kbd4", 0x03, KEYBOARD, dy=0.55)

legend_elems = [
    Line2D([0], [0], marker='o', color='w', markerfacecolor=LIGHTBAR, markersize=10, label='Chassis lightbar zone (12 zones)'),
    Line2D([0], [0], marker='o', color='w', markerfacecolor=KEYBOARD, markersize=10, label='Keyboard zone (4 zones)'),
]
ax.legend(handles=legend_elems, loc="lower center", bbox_to_anchor=(0.5, -0.06), ncol=2, frameon=False, fontsize=9)

plt.tight_layout()
plt.savefig("g615lr_zone_map.png", bbox_inches="tight", facecolor="white")
print("saved g615lr_zone_map.png")
