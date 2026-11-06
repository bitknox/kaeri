using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows;

/// <summary>
/// Art that fades in over its placeholder when it arrives (fade-in), as the spec has covers do.
/// Only art that comes while the image is on screen with nothing in it fades: art already here
/// when the image is built, or a recycled cell taking another title's cover, shows at once, so a
/// redraw never flashes the placeholder. In XAML: <c>local:ArtFade.On="True"</c> on an Image.
/// </summary>
public static class ArtFade
{
    public static readonly DependencyProperty OnProperty = DependencyProperty.RegisterAttached(
        "On", typeof(bool), typeof(ArtFade), new PropertyMetadata(false, (d, e) =>
        {
            if (d is Image image && e.NewValue is true)
                Watch(image);
        }));

    public static bool GetOn(DependencyObject d) => (bool)d.GetValue(OnProperty);

    public static void SetOn(DependencyObject d, bool value) => d.SetValue(OnProperty, value);

    static readonly DependencyProperty WasProperty = DependencyProperty.RegisterAttached(
        "Was", typeof(ImageSource), typeof(ArtFade), new PropertyMetadata(null));

    /// <summary>Fades <paramref name="image"/>'s art in whenever it arrives into an empty, loaded image.</summary>
    public static void Watch(Image image)
    {
        image.SetValue(WasProperty, image.Source);
        image.RegisterPropertyChangedCallback(Image.SourceProperty, (sender, _) =>
        {
            var at = (Image)sender;
            var was = (ImageSource?)at.GetValue(WasProperty);
            at.SetValue(WasProperty, at.Source);
            if (at.Source is null)
            {
                Motion.Set(at, true);
                return;
            }
            if (was is null && at.IsLoaded)
                Motion.FadeIn(at);
        });
    }
}
