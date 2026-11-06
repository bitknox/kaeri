using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows.Pages;

/// <summary>
/// A freshly made invite: its QR code at 160 pixels, the link, Share…, Copy link and Done, and one
/// line saying who can use it. Share opens the system's share sheet; the code is for the phone
/// beside you.
/// </summary>
static class InviteDialog
{
    public static void Show(Ctx ctx, XamlRoot root, InviteLinkView invite) =>
        Show(ctx, root, invite.Link.ToString() ?? "", invite.ForWhom);

    public static async void Show(Ctx ctx, XamlRoot root, string link, string? forWhom)
    {
        var forDevice = forWhom is null;
        var body = new StackPanel { Spacing = 12, MinWidth = 360 };
        body.Children.Add(QrCode.Make(link));
        body.Children.Add(Ui.Haze(forDevice ? T.StoresQrWarningDevice : T.StoresQrWarning));
        var box = new TextBox { Text = link, IsReadOnly = true };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, T.StoresInviteLink);
        // Copy sits beside the link, so the dialog's Close button can be Done: Escape then closes it.
        var row = new Grid { ColumnSpacing = 8 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.Children.Add(box);
        var copy = Ui.Label(T.StoresCopyLink, (_, _) => Ui.TryCopy(link));
        Grid.SetColumn(copy, 1);
        row.Children.Add(copy);
        body.Children.Add(row);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = forDevice ? T.StoresLinkDeviceTitle : forWhom is { Length: > 0 } name ? T.StoresInviteForName(name) : T.StoresInviteTitle,
            Content = body,
            PrimaryButtonText = T.StoresShare,
            CloseButtonText = T.ActionDone,
            DefaultButton = ContentDialogButton.Close,
        };
        // Share keeps the sheet up; Done, and Escape, close it.
        dialog.PrimaryButtonClick += (_, e) =>
        {
            e.Cancel = true;
            Share(ctx, link);
        };
        await Dialogs.Show(dialog);
    }

    /// <summary>The system's share sheet for the link, on the app's window.</summary>
    static void Share(Ctx ctx, string link)
    {
        try
        {
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(ctx.Window);
            var manager = global::Windows.ApplicationModel.DataTransfer.DataTransferManagerInterop.GetForWindow(hwnd);
            global::Windows.Foundation.TypedEventHandler<global::Windows.ApplicationModel.DataTransfer.DataTransferManager, global::Windows.ApplicationModel.DataTransfer.DataRequestedEventArgs>? handler = null;
            handler = (_, e) =>
            {
                manager.DataRequested -= handler;
                e.Request.Data.Properties.Title = T.StoresInviteTitle;
                e.Request.Data.SetText(link);
                e.Request.Data.SetWebLink(new Uri(link));
            };
            manager.DataRequested += handler;
            global::Windows.ApplicationModel.DataTransfer.DataTransferManagerInterop.ShowShareUIForWindow(hwnd);
        }
        catch (Exception e)
        {
            Kaeri.Engine.Ffi.KaeriFfiMethods.LogLine(Kaeri.Engine.Ffi.FfiLogLevel.Warn, "windows", $"the share sheet would not open: {e.Message}");
            Ui.TryCopy(link);
        }
    }
}
