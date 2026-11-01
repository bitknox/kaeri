using Kaeri.Engine.Ffi;
using Kaeri.Engine.Core;
using Kaeri.Engine.Model;

namespace Kaeri.Windows.Player;

/// <summary>
/// How the viewer's subtitle style reaches mpv: the engine's own <c>mpv_subtitle_options</c> with
/// the system's caption style (Settings, Accessibility, Captions) handed in, and the typeface
/// libass cannot find by itself put where it looks. The style is the engine's; nothing is merged here.
/// </summary>
static class SubtitleStyling
{
    /// <summary>The system's caption style as last read, or null when it sets none or cannot be read.</summary>
    public static SystemCaptionStyle? System { get; private set; }

    /// <summary>Said when a fresh read found the system's style changed.</summary>
    public static event Action? SystemChanged;

    /// <summary>
    /// Reads Settings, Accessibility, Captions again. The API has no change event, so this runs
    /// when the window activates. Nothing is read from a system that sets no style.
    /// </summary>
    public static void Refresh()
    {
        var now = Read();
        if (Equals(now, System))
            return;
        System = now;
        SystemChanged?.Invoke();
    }

    static SystemCaptionStyle? Read()
    {
        try
        {
            return Of(
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.FontColor,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.ComputedFontColor,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.FontOpacity,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.FontSize,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.FontStyle,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.FontEffect,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.BackgroundColor,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.ComputedBackgroundColor,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.BackgroundOpacity,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.RegionColor,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.ComputedRegionColor,
                global::Windows.Media.ClosedCaptioning.ClosedCaptionProperties.RegionOpacity);
        }
        catch (Exception)
        {
            return null;
        }
    }

    static SystemCaptionStyle? Of(
        global::Windows.Media.ClosedCaptioning.ClosedCaptionColor fontColor, global::Windows.UI.Color fontComputed,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionOpacity fontOpacity,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionSize size,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle style,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionEdgeEffect effect,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionColor backColor, global::Windows.UI.Color backComputed,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionOpacity backOpacity,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionColor regionColor, global::Windows.UI.Color regionComputed,
        global::Windows.Media.ClosedCaptioning.ClosedCaptionOpacity regionOpacity)
    {
        string? family = style switch
        {
            global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle.MonospacedWithSerifs => "Courier New",
            global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle.ProportionalWithSerifs => "Georgia",
            global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle.MonospacedWithoutSerifs => "Consolas",
            global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle.ProportionalWithoutSerifs => "Segoe UI",
            global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle.Casual => "Comic Sans MS",
            global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle.Cursive => "Segoe Script",
            global::Windows.Media.ClosedCaptioning.ClosedCaptionStyle.SmallCapitals => "Constantia",
            _ => null,
        };
        double? relative = size switch
        {
            global::Windows.Media.ClosedCaptioning.ClosedCaptionSize.FiftyPercent => 0.5,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionSize.OneHundredPercent => 1.0,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionSize.OneHundredFiftyPercent => 1.5,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionSize.TwoHundredPercent => 2.0,
            _ => null,
        };
        CaptionEdge? edge = effect switch
        {
            global::Windows.Media.ClosedCaptioning.ClosedCaptionEdgeEffect.None => CaptionEdge.Plain,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionEdgeEffect.Raised => CaptionEdge.Raised,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionEdgeEffect.Depressed => CaptionEdge.Depressed,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionEdgeEffect.Uniform => CaptionEdge.Uniform,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionEdgeEffect.DropShadow => CaptionEdge.DropShadow,
            _ => null,
        };
        uint? text = Colour(fontColor, fontComputed, fontOpacity);
        uint? back = Colour(backColor, backComputed, backOpacity);
        uint? region = Colour(regionColor, regionComputed, regionOpacity);
        if (family is null && relative is null && edge is null && text is null && back is null && region is null)
            return null;
        return new SystemCaptionStyle(family, relative, text, back, region, edge);
    }

    /// <summary>A colour as 0xRRGGBBAA, or null when the system leaves the choice to the app.</summary>
    static uint? Colour(global::Windows.Media.ClosedCaptioning.ClosedCaptionColor chosen, global::Windows.UI.Color computed, global::Windows.Media.ClosedCaptioning.ClosedCaptionOpacity opacity)
    {
        if (chosen == global::Windows.Media.ClosedCaptioning.ClosedCaptionColor.Default)
            return null;
        var alpha = opacity switch
        {
            global::Windows.Media.ClosedCaptioning.ClosedCaptionOpacity.ZeroPercent => 0,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionOpacity.TwentyFivePercent => 0x40,
            global::Windows.Media.ClosedCaptioning.ClosedCaptionOpacity.SeventyFivePercent => 0xBF,
            _ => 0xFF,
        };
        return ((uint)computed.R << 24) | ((uint)computed.G << 16) | ((uint)computed.B << 8) | (uint)alpha;
    }

    /// <summary>The folder libass finds the bundled Zen Kaku Gothic New in.</summary>
    public static string FontsDir => Path.Combine(AppContext.BaseDirectory, "Assets", "Fonts");

    /// <summary>
    /// The player ops that draw subtitles in <paramref name="style"/>, less <c>sub-pos</c>, which the
    /// controls' lift owns. The typeface mpv would otherwise keep from an earlier style is reset.
    /// </summary>
    public static PlayerOp[] Ops(SubtitleStyle style)
    {
        var options = KaeriFfiMethods.MpvSubtitleOptions(style, System, 0);
        var ops = new List<PlayerOp> { Set("sub-fonts-dir", FontsDir) };
        if (options.All(o => o.Name != "sub-font"))
            ops.Add(Set("sub-font", "sans-serif"));
        ops.AddRange(options.Where(o => o.Name != "sub-pos").Select(o => Set(o.Name, o.Value)));
        return [.. ops];
    }

    /// <summary>Where <c>sub-pos</c> lands for a lift of <paramref name="position"/> under the style's own raise.</summary>
    public static double Position(double position, SubtitleStyle? style) =>
        Math.Max(0, position - (style?.Raise ?? 0));

    static PlayerOp Set(string name, string value) => new PlayerOp.Set(name, new MpvValue.String(value));

    /// <summary>Sends every setting of <paramref name="style"/> back, as Undo of a reset does.</summary>
    public static void Restore(Ctx ctx, SubtitleStyle style)
    {
        void Str(SettingKey key, string name) => ctx.Send(new Command.SetSetting(key, new SettingValue.Str(name)));
        void Int(SettingKey key, long value) => ctx.Send(new Command.SetSetting(key, new SettingValue.Int(value)));
        Int(SettingKey.SubtitleSize, style.Size);
        Str(SettingKey.SubtitleFont, style.Font switch
        {
            SubtitleFont.ZenKakuGothicNew => "zen-kaku-gothic-new",
            SubtitleFont.Serif => "serif",
            SubtitleFont.Monospace => "monospace",
            _ => "default",
        });
        Str(SettingKey.SubtitleColour, style.Colour switch
        {
            SubtitleColour.Yellow => "yellow",
            SubtitleColour.Cyan => "cyan",
            SubtitleColour.Green => "green",
            _ => "white",
        });
        Str(SettingKey.SubtitleEdge, style.Edge switch
        {
            SubtitleEdge.DropShadow => "drop-shadow",
            SubtitleEdge.NoEdge => "none",
            _ => "outline",
        });
        Str(SettingKey.SubtitleThickness, style.Thickness switch
        {
            SubtitleThickness.Thin => "thin",
            SubtitleThickness.Thick => "thick",
            _ => "medium",
        });
        Str(SettingKey.SubtitleBackground, style.Background switch
        {
            SubtitleBackground.EachLine => "each-line",
            SubtitleBackground.AllLines => "all-lines",
            _ => "none",
        });
        Int(SettingKey.SubtitleOpacity, style.Opacity);
        Int(SettingKey.SubtitleRaise, style.Raise);
        Str(SettingKey.StyledSubtitles, style.Styled switch
        {
            StyledSubtitles.Restyle => "restyle",
            StyledSubtitles.Strip => "strip",
            _ => "keep",
        });
        ctx.Send(new Command.SetSetting(SettingKey.FollowSystemCaptions, new SettingValue.Bool(style.FollowSystem)));
    }
}
