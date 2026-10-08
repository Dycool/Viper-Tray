# Battery icon artwork

Original PNG assets copied from [Tekk-Know/RazerBatteryTaskbar](https://github.com/Tekk-Know/RazerBatteryTaskbar/tree/main/src/assets), at the user's request. Artwork credit belongs to that project and its contributors. The upstream repository does not include an explicit license for these assets; the app's GPL notice does not grant additional rights to them.

The corresponding `.bgra` files contain the same images resized to 32 by 32 pixels with Pillow's Lanczos filter, in BGRA byte order. They are embedded in the executable so the portable app needs no external icon files.

As in the reference app, battery percentages are rounded down to the nearest 10 for the icon. Exact battery percentage and charging status remain available in the tooltip and menu.
