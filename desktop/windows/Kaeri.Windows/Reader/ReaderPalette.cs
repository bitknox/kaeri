using Medium = Kaeri.Engine.Core.Medium;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Kaeri.Windows.Pages;

/// <summary>
/// The reader's colours: the dark theme the app is in, or paper. The values are the Apple
/// reader's, so a page reads the same on every app. Windows v1 is dark, so the light set only
/// comes with Paper mode and with a light page look. A contrast theme replaces both with the
/// system's colours.
/// </summary>
sealed class ReaderPalette
{
    public required Color Ground { get; init; }
    public required Color Raised { get; init; }
    public required Color Selected { get; init; }
    public required Color High { get; init; }
    public required Color Silk { get; init; }
    public required Color Haze { get; init; }
    public required Color Rule { get; init; }
    public required Color Glass { get; init; }
    public required Color Vermilion { get; init; }
    public required Color Water { get; init; }
    public required Color Amber { get; init; }
    public required Color OnAmber { get; init; }
    /// <summary>The colour round a page with the Theme background.</summary>
    public required Color Page { get; init; }
    public required bool Paper { get; init; }

    public static readonly ReaderPalette Dark = new()
    {
        Ground = Hex(0x161A2E),
        Raised = Hex(0x222638),
        Selected = Hex(0x343A57),
        High = Hex(0x2A2F45),
        Silk = Hex(0xE4E0D6),
        Haze = Hex(0x9298B5),
        Rule = Hex(0x434653),
        Glass = Hex(0x282D44, 0.74),
        Vermilion = Hex(0xE06B54),
        Water = Hex(0x7FC7D9),
        Amber = Hex(0xF2A64B),
        OnAmber = Hex(0x161A2E),
        Page = Hex(0x161A2E),
        Paper = false,
    };

    /// <summary>Paper mode: the light set on a warm page, nothing bright.</summary>
    public static readonly ReaderPalette PaperSet = new()
    {
        Ground = Hex(0xEFE9DC),
        Raised = Hex(0xE3DFD4),
        Selected = Hex(0xD6D1C4),
        High = Hex(0xDCD7CB),
        Silk = Hex(0x1E2033),
        Haze = Hex(0x4F546E),
        Rule = Hex(0xC4BFB2),
        Glass = Hex(0xFFFDF8, 0.78),
        Vermilion = Hex(0x94301F),
        Water = Hex(0x1F5D6E),
        Amber = Hex(0xF2A64B),
        OnAmber = Hex(0x1E2033),
        Page = Hex(0xEFE9DC),
        Paper = true,
    };

    /// <summary>What shows round a page: the theme's colour, or black or white as chosen.</summary>
    public Color Backdrop(string? background) => background switch
    {
        "black" => Hex(0x000000),
        "white" => Hex(0xFFFFFF),
        _ => Page,
    };

    /// <summary>The page colours a novel's look names, for the web view's ground before it paints.</summary>
    public static Color NovelGround(string? look, bool paper) => look switch
    {
        "paper" => Hex(0xEFE9DC),
        "light" => Hex(0xF7F5F0),
        "sepia" => Hex(0xE9DCC4),
        "dark" => Hex(0x161A2E),
        _ => paper ? Hex(0xEFE9DC) : Hex(0x161A2E),
    };

    /// <summary>
    /// The colours for words and buttons on a page's own ground (the end page, the opening plate,
    /// the demo's end card): the dark set on a dark ground, the light set on a light one, each
    /// with its raised fills taken from that ground so they keep their contrast on paper, light,
    /// sepia, black or white. <paramref name="paper"/> is Paper mode, which cuts motion.
    /// </summary>
    public static ReaderPalette OnGround(Color ground, bool paper)
    {
        if (Contrast() is { } contrast)
            return contrast;
        var light = Luminance(ground) > 0.4;
        var basis = light ? PaperSet : Dark;
        var toward = light ? Hex(0x000000) : Hex(0xFFFFFF);
        return new ReaderPalette
        {
            Ground = ground,
            Raised = Mix(ground, toward, light ? 0.05 : 0.06),
            Selected = Mix(ground, toward, light ? 0.12 : 0.16),
            High = Mix(ground, toward, light ? 0.08 : 0.09),
            Silk = basis.Silk,
            Haze = basis.Haze,
            Rule = Mix(ground, toward, light ? 0.18 : 0.2),
            Glass = WithAlpha(Mix(ground, toward, light ? 0.03 : 0.08), basis.Glass.A / 255.0),
            Vermilion = basis.Vermilion,
            Water = basis.Water,
            Amber = basis.Amber,
            OnAmber = basis.OnAmber,
            Page = ground,
            Paper = paper,
        };
    }

    /// <summary>
    /// A contrast theme's colours, from the system's own, while one is on: the reader keeps its
    /// layout and drops navy and amber. Null otherwise.
    /// </summary>
    public static ReaderPalette? Contrast()
    {
        try
        {
            if (!new global::Windows.UI.ViewManagement.AccessibilitySettings().HighContrast)
                return null;
            var ui = new global::Windows.UI.ViewManagement.UISettings();
            Color Sys(global::Windows.UI.ViewManagement.UIElementType type) => ui.UIElementColor(type);
            var window = Sys(global::Windows.UI.ViewManagement.UIElementType.Window);
            var text = Sys(global::Windows.UI.ViewManagement.UIElementType.WindowText);
            var face = Sys(global::Windows.UI.ViewManagement.UIElementType.ButtonFace);
            var highlight = Sys(global::Windows.UI.ViewManagement.UIElementType.Highlight);
            var onHighlight = Sys(global::Windows.UI.ViewManagement.UIElementType.HighlightText);
            return new ReaderPalette
            {
                Ground = window,
                Raised = window,
                Selected = highlight,
                High = face,
                Silk = text,
                Haze = text,
                Rule = text,
                Glass = window,
                Vermilion = text,
                Water = text,
                Amber = highlight,
                OnAmber = onHighlight,
                Page = window,
                Paper = false,
            };
        }
        catch (Exception)
        {
            return null;
        }
    }

    static double Luminance(Color c) => (0.2126 * c.R + 0.7152 * c.G + 0.0722 * c.B) / 255.0;

    static Color Mix(Color from, Color to, double amount) => Color.FromArgb(255,
        (byte)Math.Round(from.R + (to.R - from.R) * amount),
        (byte)Math.Round(from.G + (to.G - from.G) * amount),
        (byte)Math.Round(from.B + (to.B - from.B) * amount));

    public static Color Hex(uint rgb, double alpha = 1) =>
        Color.FromArgb((byte)Math.Round(alpha * 255), (byte)(rgb >> 16), (byte)(rgb >> 8 & 0xFF), (byte)(rgb & 0xFF));

    public static Color WithAlpha(Color c, double alpha) => Color.FromArgb((byte)Math.Round(alpha * 255), c.R, c.G, c.B);

    public SolidColorBrush Brush(Color c) => new(c);
}
