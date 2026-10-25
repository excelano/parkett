//! Rasterize Parkett's drawings into what Windows and the stores take: the
//! `.ico` the window and the installer use, the MSIX assets with their scale,
//! target-size and unplated variants, the Store listing logo, and the PNG
//! exports of the two shapes in `packaging/icons`. Windows takes the square
//! drawing; the rounded one is the Linux desktop's.
//!
//!     cargo run --release            # from packaging/windows/make-ico
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use resvg::tiny_skia;
use resvg::usvg;

/// The sizes the Windows shell asks for.
///
/// 16, 32, and 48 are the list, the desktop, and the tile; 256 is what the
/// extra-large view and the file properties dialog use. 20, 24, 40, 64, and
/// 128 are the same three sizes again at the display scalings Windows offers,
/// and without them the shell picks a neighbour and resamples it, which is
/// visibly softer than rendering the vector at the size wanted.
const SIZES: &[u32] = &[16, 20, 24, 32, 40, 48, 64, 128, 256];

/// Above this, an entry is stored as PNG rather than as a bitmap.
///
/// PNG entries are read by Vista and later and nothing older, and a 256-pixel
/// bitmap entry costs 256 KiB where the PNG costs a few. Below the line the
/// bitmap is kept, because it is the format every consumer of an icon
/// directory has always understood and the saving there is not worth anything.
const PNG_ABOVE: u32 = 48;

/// One of the images `AppxManifest.xml.in` names, at the size the Store wants.
struct Asset {
    stem: &'static str,
    width: u32,
    height: u32,
    /// How much of the shorter side the drawing occupies, centred.
    fill: f32,
    /// Whether the shell ever draws this one as a bare icon rather than on a
    /// tile. Only `Square44x44Logo` is, and only that one gets the target-size
    /// and unplated variants below.
    icon: bool,
}

/// The display scalings Windows offers, as the Store spells them.
///
/// Without these there is one bitmap per asset and every other scaling is an
/// upscale of it.
const SCALES: &[u32] = &[100, 125, 150, 200, 400];

/// The sizes the shell asks for when it wants an icon rather than a tile.
const TARGET_SIZES: &[u32] = &[16, 24, 32, 48, 256];

/// The three forms of each target size, and the reason this list exists.
///
/// `BackgroundColor` in `AppxManifest.xml` is `transparent`, so where Windows
/// draws a *plated* icon it fills the plate with the user's accent colour: the
/// taskbar puts the drawing on an accent-coloured square while the side-loaded
/// install draws the same icon unplated from the `.ico`. One application with
/// two faces.
///
/// An `altform-unplated` asset is what tells the shell not to plate. The light
/// variant is the same pixels: this drawing is coloured rather than monochrome,
/// so it needs no separate treatment for a light taskbar - but the qualifier
/// has to exist, or Windows 11 falls back to the plated form there.
const ALTFORMS: &[&str] = &["", "_altform-unplated", "_altform-lightunplated"];

/// The four images the manifest names, and nothing else.
///
/// The dimensions are the Store's and are not a choice. `fill` is: a tile is
/// drawn on a coloured plate and Microsoft's tile guidance leaves the icon
/// about two thirds of it, where an icon-shaped asset - the store logo, the
/// application list entry - is drawn at the size it is given and wants the
/// whole of it. The document's logo for `.parquet` is written beside these.
const ASSETS: &[Asset] = &[
    Asset { stem: "StoreLogo", width: 50, height: 50, fill: 1.00, icon: false },
    Asset { stem: "Square44x44Logo", width: 44, height: 44, fill: 1.00, icon: true },
    Asset { stem: "Square150x150Logo", width: 150, height: 150, fill: 0.66, icon: false },
    Asset { stem: "Wide310x150Logo", width: 310, height: 150, fill: 0.66, icon: false },
];

/// The sizes Partner Center's *Store logo* listing field accepts.
///
/// Both, rather than only the smaller: the field takes either and the larger is
/// what survives a future listing that wants more pixels. Neither goes in the
/// package - see the comment where they are written.
const LISTING_SIZES: &[u32] = &[1080, 2160];

/// The sizes a submission form asks for.
///
/// 1024 is App Store Connect's, 1080 and 2160 are Partner Center's *Store
/// logo*, and 256 and 512 are what a support page or a press kit wants. All
/// square: no listing field on any of the three stores takes a wide one.
const SUBMISSION_SIZES: &[u32] = &[256, 512, 1024, 1080, 2160];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut args = std::env::args_os().skip(1);
    let svg = args
        .next()
        .map_or_else(|| here.join("../../icons/parkett-square.svg"), PathBuf::from);
    let destination = args.next().map_or_else(|| here.join(".."), PathBuf::from);
    let assets = args
        .next()
        .map_or_else(|| here.join("../assets"), PathBuf::from);
    let listing = args
        .next()
        .map_or_else(|| here.join("../listing"), PathBuf::from);
    // Two directories above this one, because both shapes are for every
    // platform's submission rather than for Windows.
    let submission = args
        .next()
        .map_or_else(|| here.join("../../icons"), PathBuf::from);

    let source = std::fs::read_to_string(&svg).map_err(|e| format!("{}: {e}", svg.display()))?;
    let tree = usvg::Tree::from_data(source.as_bytes(), &usvg::Options::default())?;

    // The icon directory: the window's icon, the Start menu shortcut's, and
    // what `install.ps1` points the Add/Remove entry at. One, because there is
    // one drawing.
    std::fs::create_dir_all(&destination)?;
    let document_svg = here.join("../../linux/icons/application-vnd.apache.parquet.svg");
    let document = std::fs::read_to_string(&document_svg).map_err(|e| format!("{}: {e}", document_svg.display()))?;
    let document = usvg::Tree::from_data(document.as_bytes(), &usvg::Options::default())?;
    for (drawn, name) in [(&tree, "parkett.ico"), (&document, "parquet.ico")] {
        let path = destination.join(name);
        write_ico(drawn, &path)?;
        println!("wrote {} - {} sizes", path.display(), SIZES.len());
    }

    if assets.exists() {
        std::fs::remove_dir_all(&assets)?;
    }
    std::fs::create_dir_all(&assets)?;

    let mut written = 0usize;
    for asset in ASSETS {
        // The unqualified name as well as the qualified ones. The manifest
        // names this one, and it is what resolves when nothing indexes the
        // package - so keeping it means the assets are correct with or without
        // `resources.pri`, rather than only with.
        write_png(
            &tree,
            &assets,
            &format!("{}.png", asset.stem),
            asset.width,
            asset.height,
            asset.fill,
        )?;
        written += 1;

        for &scale in SCALES {
            let (w, h) = (scaled(asset.width, scale), scaled(asset.height, scale));
            let name = format!("{}.scale-{}.png", asset.stem, scale);
            write_png(&tree, &assets, &name, w, h, asset.fill)?;
            written += 1;
        }

        if asset.icon {
            for &size in TARGET_SIZES {
                for altform in ALTFORMS {
                    let name = format!("{}.targetsize-{}{}.png", asset.stem, size, altform);
                    write_png(&tree, &assets, &name, size, size, 1.0)?;
                    written += 1;
                }
            }
        }
    }
    write_png(&document, &assets, "parquet.png", 44, 44, 1.0)?;
    written += 1;
    for &scale in SCALES {
        let side = scaled(44, scale);
        write_png(&document, &assets, &format!("parquet.scale-{scale}.png"), side, side, 1.0)?;
        written += 1;
    }
    for &size in TARGET_SIZES {
        write_png(&document, &assets, &format!("parquet.targetsize-{size}.png"), size, size, 1.0)?;
        written += 1;
    }
    println!("wrote {} - {} images", assets.display(), written);

    // The store listing logo, which is not in the package and must not be.
    //
    // Partner Center's *Store logo* field is a listing image rather than a
    // package asset: it refuses anything but 1080x1080 or 2160x2160, including
    // the 300x300 the older documentation describes.
    // Writing it beside the package assets rather than into them is deliberate
    // - a file added to `assets` lands in the MSIX, and a package that gains a
    // file has to be certified again for an image no installed copy would ever
    // read.
    if listing.exists() {
        std::fs::remove_dir_all(&listing)?;
    }
    std::fs::create_dir_all(&listing)?;
    for &size in LISTING_SIZES {
        write_png(
            &tree,
            &listing,
            &format!("store-logo-{size}.png"),
            size,
            size,
            1.0,
        )?;
    }
    println!(
        "wrote {} - {} images",
        listing.display(),
        LISTING_SIZES.len()
    );

    // Both shapes rendered for the submission forms, from the two committed
    // drawings. Nothing here goes in a package: a store that masks what it is
    // given takes the square, a form that draws what it is handed the rounded.
    let mut exports = 0usize;
    for shape in ["square", "rounded"] {
        let path = submission.join(format!("parkett-{shape}.svg"));
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let drawn = usvg::Tree::from_data(text.as_bytes(), &usvg::Options::default())?;
        for &size in SUBMISSION_SIZES {
            write_png(&drawn, &submission, &format!("parkett-{shape}-{size}.png"), size, size, 1.0)?;
            exports += 1;
        }
    }
    println!("wrote {} - {exports} images", submission.display());
    Ok(())
}
/// An icon directory of every size in `SIZES`.
fn write_ico(tree: &usvg::Tree, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in SIZES {
        let image = icon_image(tree, size)?;
        let entry = if size > PNG_ABOVE {
            ico::IconDirEntry::encode_as_png(&image)?
        } else {
            ico::IconDirEntry::encode_as_bmp(&image)?
        };
        icon.add_entry(entry);
    }
    icon.write(std::fs::File::create(path)?)?;
    Ok(())
}

fn scaled(size: u32, scale: u32) -> u32 {
    (f64::from(size) * f64::from(scale) / 100.0).round() as u32
}

fn write_png(
    tree: &usvg::Tree,
    dir: &Path,
    name: &str,
    width: u32,
    height: u32,
    fill: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let pixmap = draw(tree, width, height, fill)?;
    std::fs::write(dir.join(name), pixmap.encode_png()?)?;
    Ok(())
}

/// The drawing centred on a canvas of the given size.
///
/// Every raster here comes through this, so the wide tile is the same rendering
/// as the square one rather than a square image somebody stretched. The drawing
/// is square and one of the canvases is not, which is the whole reason this
/// takes a width and a height rather than a size.
fn draw(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
    fill: f32,
) -> Result<tiny_skia::Pixmap, Box<dyn std::error::Error>> {
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or("a pixmap of that size could not be made")?;

    // The SVG is drawn on a 64-unit grid; this is the only scaling involved.
    #[allow(clippy::cast_precision_loss)]
    let (w, h) = (width as f32, height as f32);
    let side = w.min(h) * fill;
    let scale = side / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_translate((w - side) / 2.0, (h - side) / 2.0)
            .pre_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

/// The drawing at one size, as straight rather than premultiplied alpha.
///
/// `tiny_skia` renders into premultiplied pixels and an icon directory holds
/// straight ones. Handing the pixmap's bytes over unconverted looks right
/// everywhere the drawing is opaque and wrong along every antialiased edge.
/// The tile is opaque to its corners and so has no such edge, and the
/// conversion changes nothing on it; it is done because it is what makes a
/// drawing with any transparency in it correct, and it costs one pass. Its
/// own PNG encoder does the same conversion, which is why the assets above
/// need no equivalent.
fn icon_image(tree: &usvg::Tree, size: u32) -> Result<ico::IconImage, Box<dyn std::error::Error>> {
    let pixmap = draw(tree, size, size, 1.0)?;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for pixel in pixmap.pixels() {
        let straight = pixel.demultiply();
        rgba.extend_from_slice(&[
            straight.red(),
            straight.green(),
            straight.blue(),
            straight.alpha(),
        ]);
    }
    Ok(ico::IconImage::from_rgba_data(size, size, rgba))
}
