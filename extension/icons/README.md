# Extension Icons

Add these PNG files to this directory:

- `icon-16.png` - 16x16 pixels (required)
- `icon-48.png` - 48x48 pixels (required)
- `icon-128.png` - 128x128 pixels (required)

These icons will be displayed in the browser extension UI.

For now, you can use any PNG images. Here's a quick way to create placeholder icons using `convert` (ImageMagick):

```bash
convert -size 16x16 xc:#FF6B6B icon-16.png
convert -size 48x48 xc:#FF6B6B icon-48.png
convert -size 128x128 xc:#FF6B6B icon-128.png
```

This creates solid red squares. Feel free to replace with actual icons.
