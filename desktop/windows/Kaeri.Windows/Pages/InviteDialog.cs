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
        body.Children.Add(box);
        var dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = forDevice ? T.StoresLinkDeviceTitle : forWhom is { Length: > 0 } name ? T.StoresInviteForName(name) : T.StoresInviteTitle,
            Content = body,
            PrimaryButtonText = T.ActionDone,
            SecondaryButtonText = T.StoresShare,
            CloseButtonText = T.ActionCopy,
            DefaultButton = ContentDialogButton.Primary,
        };
        // Copy link and Share keep the sheet up; Done closes it.
        dialog.CloseButtonClick += (_, e) =>
        {
            e.Cancel = true;
            Ui.TryCopy(link);
        };
        dialog.SecondaryButtonClick += (_, e) =>
        {
            e.Cancel = true;
            Share(ctx, link);
        };
        try
        {
            await Dialogs.Show(dialog);
        }
        catch (System.Runtime.InteropServices.COMException)
        {
            // Another dialog is up: the link stays in the page's own copy.
        }
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
