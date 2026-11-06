using System.Net;
using System.Net.Sockets;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Kaeri.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Kaeri.Windows.Sync;

/// <summary>Where a pairing dialog was opened from: Devices, or first launch on a new device.</summary>
enum Origin
{
    Devices,
    FirstLaunch,
}

/// <summary>
/// The dialogs of sync, each a ContentDialog over the window: choosing a cloud folder and waiting
/// for the browser, the recovery key behind Windows Hello, restoring with it, pairing both ways,
/// a device's own page, and turning sync off. The engine's events move them on: a cloud sign-in's
/// steps turn the picker into a wait in place and then into Connecting, Setting up the folder,
/// and the recovery key or an error, each a crossfade in the same dialog. Only the UI thread
/// touches them.
/// </summary>
static partial class SyncDialogs
{
    /// <summary>Opens the cloud card's More menu; the card sets it when it draws, and the shots call it.</summary>
    public static Action? MoreOpener;

    static Ctx Ctx => Ctx.Current;
    static XamlRoot? Root => Ctx.Window.Content?.XamlRoot;

    /// <summary>The person pressed Sign out, so the sign-in the folder needs is theirs and no error.</summary>
    public static bool ByHand { get; private set; }

    /// <summary>The pane draws again: the cloud card's state moved on without an event of its own.</summary>
    public static event Action? Changed;

    static SyncFolderKind? last;
    static bool pairSaid;
    static string? typedName;
    static string? nameFile;

    /// <summary>Hears the engine for the dialogs, once, and asks how sync stands.</summary>
    public static void Install(string configDir)
    {
        nameFile = Path.Combine(configDir, "device-name");
        try
        {
            if (File.Exists(nameFile) && File.ReadAllText(nameFile).Trim() is { Length: > 0 } kept)
                typedName = kept;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "sync", $"the device name was not read: {e.Message}");
        }
        Ctx.Batch += Heard;
        Ctx.Send(new Command.LoadSync());
    }

    /// <summary>The name typed for this PC, else the machine's own.</summary>
    public static string TypedName => typedName ?? Environment.MachineName;

    /// <summary>The name this PC goes by: the group's when sync is on, else the one typed.</summary>
    public static string DeviceName => Ctx.Known.SyncGroup?.DeviceName ?? TypedName;

    /// <summary>
    /// This PC's new name, said to the group (or kept for the next pairing while sync is off).
    /// Play on TV takes it the next time its sender starts.
    /// </summary>
    public static void Rename(string typed)
    {
        var name = typed.Trim();
        if (name.Length == 0)
            return;
        if (name.Length > 64)
            name = name[..64];
        typedName = name;
        try
        {
            if (nameFile is not null)
            {
                Directory.CreateDirectory(Path.GetDirectoryName(nameFile)!);
                File.WriteAllText(nameFile, name);
            }
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            KaeriFfiMethods.LogLine(FfiLogLevel.Warn, "sync", $"the device name was not kept: {e.Message}");
        }
        Ctx.Send(new Command.SyncGroup(new SyncCommand.RenameDevice(name)));
    }

    /// <summary>The name the TV sender gives TVs: the one typed for this PC, else the word for a PC.</summary>
    public static string? KeptName(string configDir)
    {
        try
        {
            var file = Path.Combine(configDir, "device-name");
            return File.Exists(file) && File.ReadAllText(file).Trim() is { Length: > 0 } kept ? kept : null;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    public static void SignedOutByHand() => ByHand = true;

    // ---------------------------------------------------------------- building blocks

    static TextBlock Line(string text) => Ui.Text(text, "BodyTextBlockStyle");

    static TextBlock Note(string text) => Ui.Text(text, "HazeCaptionStyle");

    /// <summary>A text link in silk: the view's one amber stays on its primary button.</summary>
    static HyperlinkButton Link(string text)
    {
        var link = new HyperlinkButton { Content = text, Padding = new Thickness(0) };
        var silk = Ui.Res<Brush>("SilkBrush");
        foreach (var state in new[] { "HyperlinkButtonForeground", "HyperlinkButtonForegroundPointerOver", "HyperlinkButtonForegroundPressed" }) // i18n-skip
            link.Resources[state] = silk;
        return link;
    }

    static TextBlock ErrorLine()
    {
        var error = Ui.Text("", "BodyTextBlockStyle");
        error.Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush");
        error.Visibility = Visibility.Collapsed;
        return error;
    }

    static void SayError(TextBlock error, string? text)
    {
        error.Text = text ?? "";
        if (text is null)
            Motion.Set(error, false);
        else
            Motion.Show(error);
    }

    static ContentDialog Dialog(object title, UIElement body, double width = 432) => new()
    {
        XamlRoot = Root,
        Title = title,
        Content = new Border { Child = body, Width = width },
        DefaultButton = ContentDialogButton.Primary,
        PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle"),
    };

    static async void Present(ContentDialog dialog) => await Dialogs.Show(dialog);

    /// <summary>A dialog's title that crossfades to a new one, as the content below it does.</summary>
    sealed class Title
    {
        public readonly Grid View = new();

        public Title(string text) => Set(text);

        public void Set(string text)
        {
            var now = View.Children.OfType<TextBlock>().FirstOrDefault(t => t.Text == text);
            if (now is null)
            {
                now = new TextBlock { Text = text, Style = Ui.Res<Style>("SubtitleTextBlockStyle"), TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
                View.Children.Add(now);
            }
            Steps.Swap(View, now);
        }
    }

    /// <summary>Pages of one dialog in one cell, one shown at a time; each change is a crossfade.</summary>
    sealed class Steps
    {
        public readonly Grid View = new();
        readonly Dictionary<string, FrameworkElement> pages = [];
        public string Showing { get; private set; } = "";
        public event Action<string>? Shown;

        public void Add(string name, FrameworkElement page)
        {
            pages[name] = page;
            page.Visibility = Visibility.Collapsed;
            View.Children.Add(page);
        }

        public void Show(string name)
        {
            if (!pages.TryGetValue(name, out var page))
                return;
            var first = Showing.Length == 0;
            Showing = name;
            if (first)
                foreach (var (other, element) in pages)
                    Motion.Set(element, other == name);
            else
                Swap(View, page);
            Shown?.Invoke(name);
        }

        /// <summary>The new child fades in over the old one fading away, never through an empty frame.</summary>
        public static void Swap(Panel host, UIElement to)
        {
            if (host.Children.Count(c => c.Visibility == Visibility.Visible) == 0)
            {
                Motion.Set(to, true);
                return;
            }
            foreach (var child in host.Children)
                if (child == to)
                    Motion.Show(child);
                else if (child.Visibility == Visibility.Visible)
                    Motion.Hide(child);
        }
    }

    /// <summary>A row of a choosing list: its icon, its name and line, and a chevron.</summary>
    static ListViewItem ChoiceRow(string title, string line, string glyph, object tag)
    {
        var grid = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 10, 0, 10), Tag = tag };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(new FontIcon { Glyph = glyph, FontSize = 20, VerticalAlignment = VerticalAlignment.Center });
        var words = new StackPanel();
        words.Children.Add(Ui.Text(title, "BodyTextBlockStyle"));
        words.Children.Add(Ui.Text(line, "HazeCaptionStyle"));
        Grid.SetColumn(words, 1);
        grid.Children.Add(words);
        var chevron = new FontIcon { Glyph = "", FontSize = 12, Foreground = Ui.Res<Brush>("HazeBrush"), VerticalAlignment = VerticalAlignment.Center };
        Grid.SetColumn(chevron, 2);
        grid.Children.Add(chevron);
        var item = new ListViewItem { Content = grid, Tag = tag, HorizontalContentAlignment = HorizontalAlignment.Stretch };
        AutomationProperties.SetName(item, $"{title}, {line}");
        return item;
    }

    static ListView ChoiceList(Action<object> chosen)
    {
        var list = new ListView { SelectionMode = ListViewSelectionMode.None, IsItemClickEnabled = true, Margin = new Thickness(-12, 0, -12, 0) };
        // The row and its content carry the same tag, whichever of them the click names.
        list.ItemClick += (_, e) =>
        {
            if (e.ClickedItem is FrameworkElement { Tag: { } tag })
                chosen(tag);
        };
        return list;
    }

    static TextBox Field(string header, InputScopeNameValue scope = InputScopeNameValue.Default)
    {
        var box = new TextBox { Header = header };
        box.InputScope = new InputScope { Names = { new InputScopeName(scope) } };
        return box;
    }

    static async void OpenUrl(string url)
    {
        if (Uri.TryCreate(url, UriKind.Absolute, out var uri))
            await global::Windows.System.Launcher.LaunchUriAsync(uri);
    }

    // ---------------------------------------------------------------- picker and wait

    /// <summary>Where the sign-in stands in the picker's wait.</summary>
    enum Phase
    {
        /// <summary>The browser is open on the service's page.</summary>
        Browser,
        /// <summary>The browser came back with access allowed.</summary>
        Connecting,
        /// <summary>Signed in; the folder is being made.</summary>
        SettingUp,
        /// <summary>The sign-in or the set-up failed.</summary>
        Error,
        /// <summary>The folder is ready and the recovery key shows.</summary>
        Key,
    }

    sealed class PickerParts
    {
        public required ContentDialog Dialog;
        public required Steps Pages;
        public required Title Title;
        public required TextBlock Error, WaitTitle, WaitLine, CarriesOn, FailLine, NcError, DavError;
        public required HyperlinkButton OpenAgain;
        public required ProgressRing Ring;
        public required StackPanel KeySlot;
        public Phase State = Phase.Browser;
        public string NcHost = "";
        public string? WaitUrl;
        /// <summary>The sign-in got as far as making the folder.</summary>
        public bool Finishing;
        /// <summary>The dialog is being closed by the code, not by the person.</summary>
        public bool Done;
        /// <summary>When the browser came back, and when Connecting or Setting up was put on screen.</summary>
        public DateTime? Returned, Shown;
        /// <summary>A returning state (Connecting or Setting up) has arrived, shown yet or not.</summary>
        public bool Returning;
        public CancellationTokenSource? Pending;
    }

    static PickerParts? picker;

    /// <summary>The picker was opened by Use another service: a choice moves the group's folder.</summary>
    static bool changing;

    /// <summary>Choose a cloud folder.</summary>
    public static void Picker()
    {
        changing = false;
        PresentPicker("list", null);
    }

    /// <summary>Use another service: the picker with the folder in use marked, whose choice moves the group.</summary>
    public static void PickerChange(SyncFolderKind inUse)
    {
        changing = true;
        PresentPicker("list", inUse);
    }

    /// <summary>The wait for the browser on its own: Sign in again from the cloud card.</summary>
    public static void SignInAgain(SyncFolderKind service)
    {
        ByHand = false;
        changing = false;
        last = service;
        Ctx.Known.SyncTrouble = null;
        Ctx.Send(new Command.SyncSignIn(service));
        var p = PresentPicker("wait", null);
        StageTo(p, service, Phase.Browser);
    }

    /// <summary>Set a folder up: the first one turns sync on, and Use another service moves the group.</summary>
    static void Begin(SyncFolderSpec folder)
    {
        if (changing)
            Ctx.Send(new Command.SyncGroup(new SyncCommand.ChangeSyncFolder(folder)));
        else
            Ctx.Send(new Command.StartSync(TypedName, folder));
    }

    /// <summary>Remove the cloud folder, after asking what stays. It signs out of the service too.</summary>
    public static async void RemoveFolder(SyncFolderKind kind)
    {
        var service = SyncWords.ServiceName(kind);
        if (Root is { } root && await Ui.Confirm(root, T.CloudRemoveTitle(service), T.CloudRemoveLine(SyncWords.Kind, service), T.CloudRemoveAction))
            Ctx.Send(new Command.SyncGroup(new SyncCommand.ChangeSyncFolder(null)));
    }

    /// <summary>
    /// Puts the wait in <paramref name="stage"/> at the pace of the return: Connecting and Setting
    /// up show only past 600 ms of it, and a state shown stays at least 400 ms.
    /// </summary>
    static async void StageTo(PickerParts p, SyncFolderKind service, Phase stage, string? url = null, (string Title, string Line)? error = null)
    {
        var returning = stage is Phase.Connecting or Phase.SettingUp;
        p.Pending?.Cancel();
        p.Pending = null;
        p.Returning = returning;
        if (returning)
            Buttons(p);
        if (stage == Phase.Browser)
        {
            p.Returned = null;
            p.Shown = null;
            p.Finishing = false;
        }
        else
        {
            p.Returned ??= DateTime.UtcNow;
        }
        var delay = TimeSpan.Zero;
        if (returning && p.Shown is null && p.Returned is { } returned)
            delay = TimeSpan.FromMilliseconds(600) - (DateTime.UtcNow - returned);
        if (p.Shown is { } shown)
        {
            var left = TimeSpan.FromMilliseconds(400) - (DateTime.UtcNow - shown);
            if (left > delay)
                delay = left;
        }
        if (delay > TimeSpan.Zero)
        {
            var pending = p.Pending = new CancellationTokenSource();
            try
            {
                await Task.Delay(delay, pending.Token);
            }
            catch (TaskCanceledException)
            {
                return;
            }
        }
        p.Shown = returning ? DateTime.UtcNow : null;
        ShowStage(p, service, stage, url, error);
    }

    /// <summary>Closes the picker once the state on screen has had its 400 ms.</summary>
    static async void FinishAfterStage(PickerParts p)
    {
        p.Pending?.Cancel();
        p.Pending = null;
        if (p.Shown is { } shown)
        {
            var left = TimeSpan.FromMilliseconds(400) - (DateTime.UtcNow - shown);
            if (left > TimeSpan.Zero)
            {
                var pending = p.Pending = new CancellationTokenSource();
                try
                {
                    await Task.Delay(left, pending.Token);
                }
                catch (TaskCanceledException)
                {
                    return;
                }
            }
        }
        Close(p);
    }

    static void Close(PickerParts p)
    {
        p.Done = true;
        p.Dialog.Hide();
    }

    /// <summary>
    /// Puts the wait in <paramref name="stage"/> at once: the title and the content crossfade, the
    /// ring stays. Cancel dims once the browser has come back, as that cannot be stopped.
    /// </summary>
    static void ShowStage(PickerParts p, SyncFolderKind service, Phase stage, string? url, (string Title, string Line)? error)
    {
        var name = SyncWords.ServiceName(service);
        p.State = stage;
        var (title, line) = stage switch
        {
            Phase.Browser => (T.CloudBrowserTitle(name), T.CloudBrowserLine(name)),
            Phase.Connecting => (T.CloudConnecting(name), T.CloudConnectingLine),
            Phase.SettingUp => (T.CloudSettingUp, T.CloudSettingUpLine(name)),
            Phase.Error => error ?? ("", ""),
            _ => (T.RkeyTitle, ""),
        };
        p.WaitTitle.Text = T.CloudWaiting(name);
        p.WaitTitle.Visibility = stage == Phase.Browser ? Visibility.Visible : Visibility.Collapsed;
        p.WaitLine.Text = line;
        p.CarriesOn.Visibility = stage is Phase.Connecting or Phase.SettingUp ? Visibility.Visible : Visibility.Collapsed;
        p.FailLine.Text = line;
        p.WaitUrl = url;
        p.OpenAgain.Visibility = stage == Phase.Browser && url is not null ? Visibility.Visible : Visibility.Collapsed;
        p.Title.Set(title);
        p.Pages.Show(stage switch { Phase.Error => "error", Phase.Key => "key", _ => "wait" });
        Buttons(p);
    }

    /// <summary>The dialog's buttons for the page it shows.</summary>
    static void Buttons(PickerParts p)
    {
        var showing = p.Pages.Showing;
        var form = showing is "nextcloud" or "webdav";
        var dialog = p.Dialog;
        dialog.PrimaryButtonText = form ? T.CloudConnect : showing == "error" ? T.ActionTryAgain : showing == "key" ? T.ActionDone : "";
        dialog.DefaultButton = dialog.PrimaryButtonText.Length > 0 ? ContentDialogButton.Primary : ContentDialogButton.None;
        // Cancel is the secondary button. Once the browser is back the sign-in can't be stopped
        // half way, so it becomes Close, which leaves the work running: the recovery key or an
        // error opens by itself when it comes.
        dialog.SecondaryButtonText = showing == "key" ? "" : Dimmed(p) ? T.ActionClose : T.ActionCancel;
        dialog.IsSecondaryButtonEnabled = true;
    }

    /// <summary>The browser came back and the token fetch or the first copy cannot be stopped half way.</summary>
    static bool Dimmed(PickerParts p) => p.Pages.Showing == "wait" && (p.Returning || p.State is Phase.Connecting or Phase.SettingUp);

    /// <summary>Cancel by the person: a sign-in under way stops quietly and the picker is back as it was.</summary>
    static void CancelStep(PickerParts p)
    {
        if (p.Pages.Showing == "wait")
        {
            Ctx.Send(new Command.CancelSyncSignIn());
            Ctx.Known.SyncSigningIn = null;
        }
        p.Pending?.Cancel();
        p.Pages.Show("list");
    }

    static PickerParts PresentPicker(string page, SyncFolderKind? inUse)
    {
        if (picker is { } open)
        {
            open.Pages.Show(page);
            return open;
        }
        var status = Ctx.Known.SyncGroup;
        var pages = new Steps();

        // The list.
        var listPage = new StackPanel { Spacing = 12 };
        var error = ErrorLine();
        listPage.Children.Add(error);
        // iCloud Drive is a folder iCloud for Windows keeps in step, so it is listed only when that is set up.
        var list = ChoiceList(tag => Choose(tag is SyncFolderKind kind ? kind : SyncFolderKind.Directory));
        IReadOnlyList<(SyncFolderKind Kind, string Name)> offered = status is null ? [(SyncFolderKind.Nextcloud, "Nextcloud")] : SyncWords.Offered(status); // i18n-skip
        foreach (var (kind, name) in offered)
        {
            var line = kind switch
            {
                SyncFolderKind.Dropbox => T.CloudDropboxLine,
                SyncFolderKind.GoogleDrive => T.CloudGoogleLine,
                SyncFolderKind.OneDrive => T.CloudOnedriveLine,
                _ => T.CloudNextcloudLine,
            };
            if (ICloudDrive && string.Compare(name, ICloudName, StringComparison.OrdinalIgnoreCase) > 0 && !list.Items.OfType<ListViewItem>().Any(i => i.Tag is string))
                list.Items.Add(ChoiceRow(ICloudName, T.CloudIcloudLine, "\ue753", ICloudName));
            list.Items.Add(ChoiceRow(name, line, kind == SyncFolderKind.Nextcloud ? "" : "", kind));
        }
        if (ICloudDrive && !list.Items.OfType<ListViewItem>().Any(i => i.Tag is string))
            list.Items.Add(ChoiceRow(ICloudName, T.CloudIcloudLine, "\ue753", ICloudName));
        list.Items.Add(ChoiceRow(T.CloudWebdav, T.CloudWebdavLine, "", SyncFolderKind.WebDav));
        list.Items.Add(ChoiceRow(T.CloudFolder(SyncWords.Kind), T.CloudFolderLine, "", SyncFolderKind.Directory));
        listPage.Children.Add(list);
        listPage.Children.Add(Note(T.CloudPickFoot));
        pages.Add("list", listPage);

        // The wait: its states crossfade in the one dialog. The ring stays; the title in the
        // dialog's header and the line below it change.
        var wait = new StackPanel { Spacing = 12 };
        var waitRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        var ring = new ProgressRing { IsActive = true, Width = 20, Height = 20 };
        waitRow.Children.Add(ring);
        var waitTitle = Ui.Text("", "BodyStrongTextBlockStyle");
        waitTitle.VerticalAlignment = VerticalAlignment.Center;
        waitRow.Children.Add(waitTitle);
        wait.Children.Add(waitRow);
        var waitLine = Ui.Text("", "HazeBodyStyle");
        wait.Children.Add(waitLine);
        // Once the browser is back the work can't be stopped, but the dialog can always be closed.
        var carriesOn = Note(T.CloudCloseCarriesOn);
        carriesOn.Visibility = Visibility.Collapsed;
        wait.Children.Add(carriesOn);
        // A secondary link is silk; the dialog's one amber is its primary button.
        var openAgain = Link(T.CloudOpenAgain);
        openAgain.Visibility = Visibility.Collapsed;
        wait.Children.Add(openAgain);
        pages.Add("wait", wait);

        // A failure: what failed, with Try again and Cancel in the buttons.
        var failed = new StackPanel { Spacing = 12 };
        var failLine = Ui.Text("", "HazeBodyStyle");
        failed.Children.Add(failLine);
        pages.Add("error", failed);

        // The recovery key, once the folder is ready.
        var keySlot = new StackPanel { Spacing = 16 };
        pages.Add("key", keySlot);

        // Nextcloud.
        var nc = new StackPanel { Spacing = 12 };
        var ncError = ErrorLine();
        var address = Field(T.CloudNextcloudAddress, InputScopeNameValue.Url);
        var user = Field(T.CloudUserLabel);
        var password = new PasswordBox { Header = T.CloudPasswordLabel };
        var appGroup = new StackPanel { Spacing = 12, Visibility = Visibility.Collapsed };
        appGroup.Children.Add(user);
        appGroup.Children.Add(password);
        var ncNote = Note(T.CloudNextcloudAddressNote);
        var appPassword = Link(T.CloudAppPassword);
        appPassword.Click += (_, _) =>
        {
            Motion.Show(appGroup);
            ncNote.Text = T.CloudAppPasswordNote(SyncWords.Kind);
            appPassword.Visibility = Visibility.Collapsed;
        };
        nc.Children.Add(ncError);
        nc.Children.Add(address);
        nc.Children.Add(ncNote);
        nc.Children.Add(appGroup);
        nc.Children.Add(appPassword);
        pages.Add("nextcloud", nc);

        // WebDAV.
        var dav = new StackPanel { Spacing = 12 };
        var davError = ErrorLine();
        var davAddress = Field(T.CloudWebdavAddressLabel, InputScopeNameValue.Url);
        var davUser = Field(T.CloudUserLabel);
        var davPassword = new PasswordBox { Header = T.CloudPasswordLabel };
        dav.Children.Add(davError);
        dav.Children.Add(davAddress);
        dav.Children.Add(davUser);
        dav.Children.Add(davPassword);
        pages.Add("webdav", dav);

        var title = new Title(T.CloudPickTitle);
        // A form page has a back glyph before its title, which already names the service.
        var back = new Button
        {
            Content = new FontIcon { Glyph = "\uE72B", FontSize = 14 },
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            VerticalAlignment = VerticalAlignment.Center,
            Visibility = Visibility.Collapsed,
        };
        AutomationProperties.SetName(back, T.ActionBack);
        ToolTipService.SetToolTip(back, T.ActionBack);
        back.Click += (_, _) => pages.Show("list");
        var titleRow = new Grid { ColumnSpacing = 8 };
        titleRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        titleRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        titleRow.Children.Add(back);
        Grid.SetColumn(title.View, 1);
        titleRow.Children.Add(title.View);
        var dialog = Dialog(titleRow, pages.View, 456);
        dialog.DefaultButton = ContentDialogButton.None;
        var parts = new PickerParts
        {
            Dialog = dialog,
            Pages = pages,
            Title = title,
            Error = error,
            WaitTitle = waitTitle,
            WaitLine = waitLine,
            CarriesOn = carriesOn,
            FailLine = failLine,
            Ring = ring,
            KeySlot = keySlot,
            OpenAgain = openAgain,
            NcError = ncError,
            DavError = davError,
        };
        openAgain.Click += (_, _) =>
        {
            if (parts.WaitUrl is { } url)
                OpenUrl(url);
        };
        // Connect is the dialog's primary button on the Nextcloud and WebDAV pages; it keeps the
        // dialog open, and the engine's answer moves it on.
        void ConnectNextcloud()
        {
            var typed = address.Text.Trim();
            if (typed.Length == 0)
                return;
            parts.NcHost = SyncWords.HostOf(typed);
            last = SyncFolderKind.Nextcloud;
            Ctx.Known.SyncTrouble = null;
            SayError(ncError, null);
            var withPassword = appGroup.Visibility == Visibility.Visible;
            Begin(new SyncFolderSpec(SyncFolderKind.Nextcloud, typed, withPassword ? user.Text : null, withPassword ? password.Password : null));
        }
        void ConnectWebdav()
        {
            if (davAddress.Text.Trim().Length == 0 || davUser.Text.Length == 0)
                return;
            last = SyncFolderKind.WebDav;
            Ctx.Known.SyncTrouble = null;
            SayError(davError, null);
            Begin(new SyncFolderSpec(SyncFolderKind.WebDav, davAddress.Text.Trim(), davUser.Text, davPassword.Password));
        }
        dialog.PrimaryButtonClick += (_, e) =>
        {
            if (parts.State == Phase.Key && pages.Showing == "key")
            {
                // Done closes the dialog; the key is not offered again by itself.
                Ctx.Known.RecoveryKey = null;
                parts.Done = true;
                return;
            }
            e.Cancel = true;
            if (pages.Showing == "nextcloud")
                ConnectNextcloud();
            else if (pages.Showing == "webdav")
                ConnectWebdav();
            else if (pages.Showing == "error" && last is { } again)
                Choose(again);
        };
        // The one place Cancel means two things: from the list it closes, from any later step it
        // stops the sign-in quietly and the picker is back as it was.
        dialog.SecondaryButtonClick += (_, e) =>
        {
            if (pages.Showing == "list")
                return;
            if (Dimmed(parts))
            {
                // Close: the work carries on without the dialog.
                parts.Done = true;
                return;
            }
            e.Cancel = true;
            CancelStep(parts);
        };
        // Escape is Cancel: it closes the list and goes back from a later step. While the work
        // can't be stopped it closes the dialog and leaves the work running, and on the key it
        // is Done, as the key has been on screen. A close the app asks for always goes through.
        dialog.Closing += (sender, e) =>
        {
            if (parts.Done || Dialogs.Forced(sender) || e.Result != ContentDialogResult.None || pages.Showing == "list")
                return;
            if (pages.Showing == "key")
            {
                Ctx.Known.RecoveryKey = null;
                return;
            }
            if (Dimmed(parts))
                return;
            e.Cancel = true;
            CancelStep(parts);
        };
        pages.Shown += name =>
        {
            var form = name is "nextcloud" or "webdav";
            back.Visibility = form ? Visibility.Visible : Visibility.Collapsed;
            Buttons(parts);
            switch (name)
            {
                case "list":
                    title.Set(T.CloudPickTitle);
                    break;
                case "nextcloud":
                    title.Set("Nextcloud"); // i18n-skip
                    break;
                case "webdav":
                    title.Set(T.CloudWebdav);
                    break;
            }
        };
        dialog.Closed += (_, _) =>
        {
            parts.Pending?.Cancel();
            if (picker == parts)
                picker = null;
        };
        pages.Show(page);
        picker = parts;
        Present(dialog);
        return parts;
    }

    /// <summary>The row of the service in use, in Use another service: dimmed and marked.</summary>
    static ListViewItem InUse(ListViewItem row, SyncFolderKind kind, SyncFolderKind? inUse)
    {
        if (inUse != kind)
            return row;
        row.IsEnabled = false;
        if (row.Content is Grid { Children: { Count: > 1 } children } && children[1] is StackPanel { Children: { Count: > 1 } words } && words[1] is TextBlock line)
            line.Text = T.CloudInUse;
        return row;
    }

    const string ICloudName = "iCloud Drive"; // i18n-skip

    /// <summary>iCloud for Windows is set up: it keeps an iCloud Drive folder in the user's folder.</summary>
    static bool ICloudDrive =>
        Directory.Exists(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "iCloudDrive"));

    /// <summary>A row in the picker was chosen.</summary>
    static async void Choose(SyncFolderKind kind)
    {
        Ctx.Known.SyncTrouble = null;
        last = kind;
        ByHand = false;
        if (picker is not { } p)
            return;
        SayError(p.Error, null);
        switch (kind)
        {
            case SyncFolderKind.GoogleDrive or SyncFolderKind.Dropbox or SyncFolderKind.OneDrive:
                Begin(new SyncFolderSpec(kind));
                StageTo(p, kind, Phase.Browser);
                break;
            case SyncFolderKind.Nextcloud:
                p.Pages.Show("nextcloud");
                break;
            case SyncFolderKind.WebDav:
                p.Pages.Show("webdav");
                break;
            default:
                if (await Ctx.Window.PickFolder() is { Length: > 0 } path)
                    Begin(new SyncFolderSpec(SyncFolderKind.Directory, path));
                break;
        }
    }

    // ---------------------------------------------------------------- recovery key

    static ContentDialog? keyDialog;

    /// <summary>
    /// The recovery key's content, into <paramref name="body"/>: its line, the key in groups of
    /// four, Copy, Save and the QR code, and the note on how to see it again.
    /// </summary>
    static void FillKey(StackPanel body, string key)
    {
        body.Children.Clear();
        body.Children.Add(Ui.Text(T.RkeyLine, "HazeBodyStyle"));
        var groups = SyncWords.KeyGroups(key);
        var grid = new StackPanel { Spacing = 6 };
        foreach (var row in groups.Chunk(3))
            grid.Children.Add(new TextBlock
            {
                Text = string.Join("  ", row),
                FontFamily = new FontFamily("Cascadia Mono, Consolas"),
                FontSize = 18,
                IsTextSelectionEnabled = true,
                HorizontalAlignment = HorizontalAlignment.Center,
            });
        var plate = new Border
        {
            Child = grid,
            Padding = new Thickness(16),
            CornerRadius = new CornerRadius(4),
            Background = Ui.Res<Brush>("GroundBrush"),
            BorderBrush = Ui.Res<Brush>("EdgeBrush"),
            BorderThickness = new Thickness(1),
        };
        AutomationProperties.SetName(plate, $"{T.RkeyTitle}: {string.Join(" ", groups)}");
        body.Children.Add(plate);
        var qr = QrCode.Make(string.Concat(groups), 168);
        qr.HorizontalAlignment = HorizontalAlignment.Center;
        qr.Visibility = Visibility.Collapsed;
        body.Children.Add(qr);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        Button WithIcon(string glyph, string text)
        {
            var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            content.Children.Add(new FontIcon { Glyph = glyph, FontSize = 16 });
            content.Children.Add(new TextBlock { Text = text });
            var button = new Button { Content = content };
            AutomationProperties.SetName(button, text);
            buttons.Children.Add(button);
            return button;
        }
        var words = string.Join(" ", groups);
        WithIcon("", T.RkeyCopy).Click += (_, _) => Ui.TryCopy(words);
        WithIcon("", T.RkeySaveFile).Click += async (_, _) =>
        {
            var save = new global::Windows.Storage.Pickers.FileSavePicker { SuggestedFileName = "kaeri-recovery-key" }; // i18n-skip
            save.FileTypeChoices.Add(T.RkeyTitle, [".txt"]);
            WinRT.Interop.InitializeWithWindow.Initialize(save, WinRT.Interop.WindowNative.GetWindowHandle(Ctx.Window));
            if (await save.PickSaveFileAsync() is { } file)
                await global::Windows.Storage.FileIO.WriteTextAsync(file, words + "\n");
        };
        WithIcon("", T.RkeyQrShort).Click += (_, _) =>
        {
            if (qr.Visibility == Visibility.Visible)
                Motion.Hide(qr);
            else
                Motion.Show(qr);
        };
        body.Children.Add(buttons);
        body.Children.Add(Note(T.RkeyLockWindows));
    }

    /// <summary>The recovery key on its own, when it is seen again from Devices. The first time, it shows in the picker's dialog.</summary>
    public static void KeyDialog()
    {
        if (Ctx.Known.RecoveryKey is not { } key)
            return;
        if (keyDialog is { } before)
        {
            keyDialog = null;
            Dialogs.Close(before);
        }
        var body = new StackPanel { Spacing = 16 };
        FillKey(body, key);
        var dialog = Dialog(T.RkeyTitle, body, 432);
        dialog.PrimaryButtonText = T.ActionDone;
        // Escape is Done: the key has been on screen, and Devices shows it again behind the lock.
        dialog.Closing += (_, _) => Ctx.Known.RecoveryKey = null;
        dialog.Closed += (_, _) =>
        {
            if (keyDialog == dialog)
                keyDialog = null;
        };
        keyDialog = dialog;
        Present(dialog);
    }

    /// <summary>Show the key again, after Windows Hello where it is set up.</summary>
    public static void ShowKeyAgain() => HelloLock.Ask(() => Ctx.Send(new Command.ShowRecoveryKey()));

    /// <summary>Make a new recovery key, after Windows Hello where it is set up.</summary>
    public static void MakeNewKey() => HelloLock.Ask(() => Ctx.Send(new Command.NewRecoveryKey()));

    sealed record RestoreParts(ContentDialog Dialog, TextBlock Error);

    static RestoreParts? restore;

    /// <summary>Restore: a folder that holds history asks for its recovery key.</summary>
    public static void Restore(SyncFolderKind service)
    {
        restore?.Dialog.Hide();
        var count = Ctx.Known.SyncGroup is { } s ? Math.Max(1, s.Devices.Length) : 0;
        var body = new StackPanel { Spacing = 12 };
        body.Children.Add(Ui.Text(T.RkeyEnterLine(SyncWords.ServiceName(service), Math.Max(2, count)), "HazeBodyStyle"));
        var entry = Field(T.RkeyFieldLabel);
        // The key is set in mono; the label above it keeps the dialog's own type.
        entry.Header = new TextBlock { Text = T.RkeyFieldLabel, FontFamily = (FontFamily)Application.Current.Resources["ContentControlThemeFontFamily"] };
        AutomationProperties.SetName(entry, T.RkeyFieldLabel);
        entry.FontFamily = new FontFamily("Cascadia Mono, Consolas");
        body.Children.Add(entry);
        body.Children.Add(Note(T.RkeyEnterNote));
        var error = ErrorLine();
        body.Children.Add(error);
        var dialog = Dialog(T.RkeyEnterTitle, body);
        dialog.PrimaryButtonText = T.RkeyRestore;
        dialog.CloseButtonText = T.ActionCancel;
        dialog.IsPrimaryButtonEnabled = false;
        entry.TextChanged += (_, _) => dialog.IsPrimaryButtonEnabled = SyncWords.KeyReady(entry.Text);
        dialog.PrimaryButtonClick += (_, e) =>
        {
            // Open until the engine answers: a wrong key says so here.
            e.Cancel = true;
            SayError(error, null);
            Ctx.Known.SyncTrouble = null;
            Ctx.Send(new Command.JoinSync(TypedName, new SyncFolderSpec(service), entry.Text));
        };
        var parts = new RestoreParts(dialog, error);
        dialog.Closed += (_, _) =>
        {
            if (restore == parts)
                restore = null;
        };
        restore = parts;
        Present(dialog);
    }

    // ---------------------------------------------------------------- pairing

    /// <summary>This PC's address on the local network, for the code the other device types or scans.</summary>
    static string LocalAddress()
    {
        try
        {
            using var socket = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            // Nothing is sent: connecting a datagram socket only picks the route.
            socket.Connect("192.0.2.1", 9);
            if (socket.LocalEndPoint is IPEndPoint { } local)
                return local.Address.ToString();
        }
        catch (SocketException)
        {
        }
        return "127.0.0.1";
    }

    sealed class PairParts
    {
        public required ContentDialog Dialog;
        public required Steps Pages;
        public required TextBlock Code, Left, Address;
        public required Title PairedTitle;
        public required Border QrSlot;
        public required StackPanel CopySlot;
        public required Origin Origin;
        public required int StartedWith;
        public required Microsoft.UI.Dispatching.DispatcherQueueTimer Clock;
        public string? QrFor;
    }

    static PairParts? pair;

    /// <summary>Add a device by showing a code for the other one to enter.</summary>
    public static void PairShow(Origin origin)
    {
        Ctx.Known.PairInvite = null;
        Ctx.Send(new Command.PairInvite(TypedName));
        PresentPairShow(origin);
    }

    public static void PresentPairShow(Origin origin)
    {
        pair?.Dialog.Hide();
        var pages = new Steps();
        var showing = new StackPanel { Spacing = 12 };
        showing.Children.Add(Ui.Text(origin == Origin.FirstLaunch ? T.FirstSetupLine : $"{T.PairShowTitle}. {T.PairShowScan}", "HazeBodyStyle"));
        var codeRow = new Grid { ColumnSpacing = 24 };
        codeRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        codeRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var qrSlot = new Border { Width = 148, Height = 148, Child = QrCode.Make("", 148) };
        codeRow.Children.Add(qrSlot);
        var words = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        var code = new TextBlock
        {
            Text = "––– –––",
            FontFamily = new FontFamily("Cascadia Mono, Consolas"),
            FontSize = 40,
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
            IsTextSelectionEnabled = true,
        };
        var left = Ui.Text("", "HazeCaptionStyle");
        var address = Ui.Text("", "HazeCaptionStyle");
        words.Children.Add(code);
        words.Children.Add(left);
        words.Children.Add(address);
        Grid.SetColumn(words, 1);
        codeRow.Children.Add(words);
        showing.Children.Add(codeRow);
        pages.Add("showing", showing);
        var ok = new StackPanel { Spacing = 12 };
        ok.Children.Add(Ui.Text(origin == Origin.FirstLaunch ? T.FirstOkLine : T.PairOkLine, "HazeBodyStyle"));
        var copySlot = new StackPanel();
        ok.Children.Add(copySlot);
        pages.Add("paired", ok);
        pages.Show("showing");
        // The title crossfades to "Paired with ..." with the page below it.
        var pairedTitle = new Title(T.DevicesAdd);
        var dialog = Dialog(pairedTitle.View, pages.View);
        dialog.PrimaryButtonText = T.ActionDone;
        var clock = Ctx.Window.DispatcherQueue.CreateTimer();
        clock.Interval = TimeSpan.FromSeconds(1);
        var parts = new PairParts
        {
            Dialog = dialog,
            Pages = pages,
            Code = code,
            Left = left,
            Address = address,
            PairedTitle = pairedTitle,
            QrSlot = qrSlot,
            CopySlot = copySlot,
            Origin = origin,
            StartedWith = Ctx.Known.SyncGroup?.Devices.Length ?? 0,
            Clock = clock,
        };
        clock.Tick += (_, _) => RefreshPair(parts);
        clock.Start();
        dialog.Closed += (_, _) =>
        {
            clock.Stop();
            if (pair == parts)
                pair = null;
            // Always: a device that stays advertised lists after its code has run out.
            Ctx.Send(new Command.PairStop());
            Ctx.Known.PairInvite = null;
        };
        pair = parts;
        RefreshPair(parts);
        Present(dialog);
    }

    static void RefreshPair(PairParts p)
    {
        if (Ctx.Known.PairInvite is not { } invite)
            return;
        var left = invite.Until - DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        p.Code.Text = SyncWords.SpacedCode(invite.Code);
        var address = $"{LocalAddress()}:{invite.Port}";
        p.Address.Text = address;
        // Drawn again only when the code changed.
        if (p.QrFor != invite.Code)
        {
            p.QrFor = invite.Code;
            p.QrSlot.Child = QrCode.Make(SyncWords.PairLink(invite.Code, address), 148);
        }
        if (left <= 0)
        {
            p.Left.Text = T.PairErrExpired;
            p.Left.Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush");
        }
        else
        {
            p.Left.Text = T.PairGoodFor(SyncWords.MinutesSeconds(left));
        }
    }

    sealed class EnterParts
    {
        public required ContentDialog Dialog;
        public required Steps Pages;
        public required TextBlock Error;
        public required Title PairedTitle;
        public required StackPanel CopySlot;
        public bool Working;
        public bool Paired;
        /// <summary>Shows the address field, as Pair by address does.</summary>
        public Action? ByAddress;
    }

    static EnterParts? enter;

    /// <summary>
    /// Add a device by entering the code the other one shows: six boxes, and an address only for
    /// a device this network's DNS-SD cannot find.
    /// </summary>
    public static void PairEnter()
    {
        enter?.Dialog.Hide();
        var pages = new Steps();
        var body = new StackPanel { Spacing = 12 };
        body.Children.Add(Ui.Text(T.PairEnterLine, "HazeBodyStyle"));
        var digits = new CodeBoxes();
        body.Children.Add(digits.View);
        var error = ErrorLine();
        body.Children.Add(error);
        var sameWifi = Note(T.PairSameWifi);
        sameWifi.HorizontalAlignment = HorizontalAlignment.Center;
        body.Children.Add(sameWifi);
        // A failed pairing already says where to look; the Wi-Fi line steps aside for it.
        error.RegisterPropertyChangedCallback(TextBlock.TextProperty, (_, _) =>
            sameWifi.Visibility = error.Text.Length == 0 ? Visibility.Visible : Visibility.Collapsed);
        // For networks where the device can't be found by itself: the address, behind its own link.
        var address = Field(T.PairAddressLabel, InputScopeNameValue.Url);
        address.PlaceholderText = "192.0.2.4"; // i18n-skip
        var byAddress = new StackPanel { Spacing = 12, Visibility = Visibility.Collapsed };
        byAddress.Children.Add(address);
        byAddress.Children.Add(Note(T.PairAddressOptional));
        body.Children.Add(byAddress);
        var links = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 24 };
        var pairByAddress = Link(T.PairByAddress);
        void OpenAddress()
        {
            pairByAddress.Visibility = Visibility.Collapsed;
            Motion.Show(byAddress);
            address.Focus(FocusState.Programmatic);
        }
        pairByAddress.Click += (_, _) => OpenAddress();
        var showInstead = Link(T.PairShowInstead);
        links.Children.Add(showInstead);
        links.Children.Add(pairByAddress);
        body.Children.Add(links);
        pages.Add("entering", body);
        var ok = new StackPanel { Spacing = 12 };
        ok.Children.Add(Ui.Text(T.PairOkLine, "HazeBodyStyle"));
        var copySlot = new StackPanel();
        ok.Children.Add(copySlot);
        pages.Add("paired", ok);
        pages.Show("entering");
        var pairedTitle = new Title(T.DevicesAdd);
        var dialog = Dialog(pairedTitle.View, pages.View);
        dialog.PrimaryButtonText = T.PairButton;
        dialog.CloseButtonText = T.ActionCancel;
        dialog.IsPrimaryButtonEnabled = false;
        var parts = new EnterParts { Dialog = dialog, Pages = pages, Error = error, PairedTitle = pairedTitle, CopySlot = copySlot, ByAddress = OpenAddress };
        digits.Changed += () =>
        {
            if (!parts.Working && !parts.Paired)
                dialog.IsPrimaryButtonEnabled = SyncWords.CodeReady(digits.Text);
        };
        showInstead.Click += (_, _) =>
        {
            dialog.Hide();
            PairShow(Origin.Devices);
        };
        dialog.PrimaryButtonClick += (_, e) =>
        {
            if (parts.Paired)
                return;
            e.Cancel = true;
            SayError(error, null);
            Ctx.Known.SyncTrouble = null;
            parts.Working = true;
            dialog.PrimaryButtonText = T.PairWorkingLine;
            dialog.IsPrimaryButtonEnabled = false;
            pairSaid = false;
            var code = SyncWords.CodeDigits(digits.Text);
            var at = address.Text.Trim();
            // Without an address the engine finds the device showing the code on the network.
            Ctx.Send(at.Length == 0
                ? new Command.SyncGroup(new SyncCommand.PairWithCode(TypedName, code))
                : new Command.PairJoin(TypedName, at, code));
        };
        dialog.Opened += (_, _) => digits.Focus();
        dialog.Closed += (_, _) =>
        {
            if (enter == parts)
                enter = null;
        };
        enter = parts;
        Present(dialog);
    }

    static void PairFailed(EnterParts e, string text)
    {
        e.Working = false;
        SayError(e.Error, text);
        e.Dialog.PrimaryButtonText = T.PairButton;
        e.Dialog.IsPrimaryButtonEnabled = true;
    }

    /// <summary>
    /// A pairing finished: the dialog's content crossfades to the success, with the Copy sign-ins
    /// switch when the other device holds sign-ins this one could take.
    /// </summary>
    static void Paired(string device)
    {
        var other = Ctx.Known.SyncGroup?.Devices.FirstOrDefault(d => !d.ThisDevice && d.Name == device);
        if (pair is { } p)
        {
            p.PairedTitle.Set(T.PairOk(device));
            // First launch asks for them from the start; adding from Devices leaves them off.
            FillCopy(p.CopySlot, other, p.Origin == Origin.FirstLaunch);
            p.Clock.Stop();
            p.Pages.Show("paired");
        }
        if (enter is { } e)
        {
            e.PairedTitle.Set(T.PairOk(device));
            FillCopy(e.CopySlot, other, false);
            e.Working = false;
            e.Paired = true;
            e.Pages.Show("paired");
            e.Dialog.PrimaryButtonText = T.ActionDone;
            e.Dialog.IsPrimaryButtonEnabled = true;
            e.Dialog.CloseButtonText = "";
        }
    }

    static void FillCopy(StackPanel slot, SyncDeviceView? device, bool on)
    {
        // Filled once: the engine says the group again while the success shows.
        if (slot.Children.Count > 0 || device is null || CopySwitch(device, on) is not { } card)
            return;
        slot.Children.Add(card);
    }

    /// <summary>The Copy sign-ins switch for a device, when it holds sign-ins.</summary>
    public static FrameworkElement? CopySwitch(SyncDeviceView device, bool on)
    {
        if (device.Trackers.Length == 0 && device.SignIns.Length == 0)
            return null;
        var toggle = new ToggleSwitch { IsOn = on || device.Asked.Length > 0, OnContent = "", OffContent = "", MinWidth = 0 };
        AutomationProperties.SetName(toggle, T.PairCopySignins(device.Name));
        void Ask(bool yes) =>
            Ctx.Send(new Command.SyncGroup(new SyncCommand.CopySignIns(device.Id, yes ? SyncWords.CopyAll(device) : [], null)));
        if (on && device.Asked.Length == 0)
            Ask(true);
        toggle.Toggled += (_, _) => Ask(toggle.IsOn);
        var card = new CommunityToolkit.WinUI.Controls.SettingsCard
        {
            Header = T.PairCopySignins(device.Name),
            Description = T.PairCopySigninsLine(SyncWords.SignInNames(device)),
            Content = toggle,
        };
        return Inline(card);
    }

    /// <summary>A dialog is narrower than a card's wrap width; this keeps the switch beside its words.</summary>
    static CommunityToolkit.WinUI.Controls.SettingsCard Inline(CommunityToolkit.WinUI.Controls.SettingsCard card)
    {
        card.Resources["SettingsCardWrapThreshold"] = 0.0;
        card.Resources["SettingsCardWrapNoIconThreshold"] = 0.0;
        return card;
    }

    // ---------------------------------------------------------------- a device's page, and leaving

    /// <summary>
    /// One device's page: its Sync switch, its Remote switch when it and this PC are a TV and a
    /// device that drives TVs, Copy sign-ins, and Remove this device.
    /// </summary>
    public static void DevicePage(SyncDeviceView device)
    {
        var body = new StackPanel { Spacing = 4 };
        CommunityToolkit.WinUI.Controls.SettingsCard Switch(string title, bool on, Action<bool> said)
        {
            var toggle = new ToggleSwitch { IsOn = on, OnContent = "", OffContent = "", MinWidth = 0 };
            AutomationProperties.SetName(toggle, title);
            toggle.Toggled += (_, _) => said(toggle.IsOn);
            return Inline(new CommunityToolkit.WinUI.Controls.SettingsCard { Header = title, Content = toggle });
        }
        body.Children.Add(Switch(T.DevicesSyncSwitch, !device.Paused, on => Ctx.Send(new Command.SyncGroup(new SyncCommand.SetDeviceSync(device.Id, on)))));
        // Only for a pair that is a TV and a device that drives TVs.
        if (device.RemoteKey is not null)
            body.Children.Add(Switch(T.DevicesRemoteSwitch, !device.RemoteOff, on => Ctx.Send(new Command.SyncGroup(new SyncCommand.SetDeviceRemote(device.Id, on)))));
        var foot = Note(device.RemoteKey is null ? T.DevicesSyncFoot : T.DevicesSwitchesFoot(SyncWords.Kind, device.Name));
        foot.Margin = new Thickness(2, 4, 0, 8);
        body.Children.Add(foot);
        if (CopySwitch(device, false) is { } copy)
        {
            copy.Margin = new Thickness(0, 0, 0, 8);
            body.Children.Add(copy);
        }
        var remove = new Button { Content = T.DevicesRemove, Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush") };
        body.Children.Add(remove);
        var dialog = Dialog(device.Name, body);
        dialog.CloseButtonText = T.ActionDone;
        dialog.DefaultButton = ContentDialogButton.Close;
        remove.Click += (_, _) =>
        {
            dialog.Hide();
            RemoveDevice(device);
        };
        Present(dialog);
    }

    /// <summary>Remove a device from the group, after asking.</summary>
    static async void RemoveDevice(SyncDeviceView device)
    {
        if (Root is { } root && await Ui.Confirm(root, T.DevicesRemoveAlert(device.Name), T.DevicesRemoveAlertLine(device.Name), T.DevicesRemove))
            Ctx.Send(new Command.RemoveSyncDevice(device.Id));
    }

    /// <summary>Turn off sync on this PC, after asking.</summary>
    public static async void TurnOff()
    {
        if (Root is { } root && await Ui.Confirm(root, T.SyncTurnOffAlert(DeviceName), T.SyncTurnOffAlertLine(SyncWords.Kind), T.DevicesTurnOffAction))
            Ctx.Send(new Command.LeaveSync());
    }

    /// <summary>Mark a kept watch unwatched after all, and let the note go.</summary>
    public static void UndoNote(SyncNoteView note)
    {
        Ctx.Send(new Command.SetWatched(note.AnilistId, [note.Episode], false));
        Ctx.Send(new Command.DismissSyncNote(note.Id));
    }

    // ---------------------------------------------------------------- events

    /// <summary>Move the dialogs on with the engine.</summary>
    static void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
            Ctx.Guarded("sync", () => Heard(e));
    }

    static void Heard(Event e)
    {
        switch (e)
        {
            case Event.SyncSignIn { Step: SyncSignInStep.Browser browser } signIn:
                if (picker is { } waiting)
                    StageTo(waiting, signIn.Service, Phase.Browser, browser.Url);
                break;
            case Event.SyncSignIn { Step: SyncSignInStep.Connecting } connecting:
                if (picker is { } returned)
                    StageTo(returned, connecting.Service, Phase.Connecting);
                break;
            case Event.SyncSignIn { Step: SyncSignInStep.Finishing } finishing:
                if (picker is { } making)
                {
                    making.Finishing = true;
                    StageTo(making, finishing.Service, Phase.SettingUp);
                }
                break;
            case Event.SyncSignIn { Step: null }:
                if (Ctx.Known.SyncTrouble is null && picker is { State: not Phase.Key, Pages.Showing: "wait" } over)
                    FinishAfterStage(over);
                break;
            case Event.SyncRecoveryKey recovery:
                if (picker is { } making2)
                {
                    // The folder was made from the picker: the key shows in the same dialog.
                    FillKey(making2.KeySlot, recovery.V1);
                    // The key's, from now: the sign-in's end that follows doesn't close the dialog first.
                    making2.State = Phase.Key;
                    StageTo(making2, last ?? SyncFolderKind.GoogleDrive, Phase.Key);
                }
                else
                {
                    KeyDialog();
                }
                break;
            case Event.SyncProblem problem:
                Problem(problem.V1);
                break;
            case Event.SyncStatus status:
                if (status.V1.Enabled && restore is { } restored)
                    restored.Dialog.Hide();
                // A pairing finished when the group has a device it had not.
                if (pair is { } p && status.V1.Devices.Length > p.StartedWith && SyncWords.Newest(status.V1) is { } added)
                    Paired(added.Name);
                else if (enter is { Working: true } && SyncWords.Newest(status.V1) is { } joined)
                    Paired(joined.Name);
                break;
            case Event.SyncPairFailed failed:
                if (enter is { } entering)
                {
                    PairFailed(entering, failed.Problem == SyncPairProblem.WrongCode
                        ? T.PairErrCode(failed.Device ?? T.PairOtherDevice)
                        : T.PairErrNotFound);
                    pairSaid = true;
                }
                break;
            case Event.SyncInvite:
                if (pair is { } showing)
                    RefreshPair(showing);
                break;
        }
    }

    static void Problem(SyncProblem problem)
    {
        var text = SyncWords.ProblemText(problem, last, picker?.NcHost ?? "");
        switch (problem)
        {
            case SyncProblem.GroupExists:
                if (picker is { } closing)
                    Close(closing);
                Restore(last ?? SyncFolderKind.GoogleDrive);
                return;
            case SyncProblem.WrongRecoveryKey:
                if (restore is { } r)
                    SayError(r.Error, text);
                return;
        }
        if (enter is { Working: true } e)
        {
            var shown = problem switch
            {
                SyncProblem.Unreachable => T.PairErrNotFound,
                SyncProblem.Other => T.PairErrCode(T.PairOtherDevice),
                _ => text,
            };
            if (!pairSaid)
                PairFailed(e, shown);
            pairSaid = false;
        }
        if (picker is { } p)
        {
            if (last is SyncFolderKind.GoogleDrive or SyncFolderKind.Dropbox or SyncFolderKind.OneDrive)
            {
                // The error is said in the dialog, with Try again.
                StageTo(p, last.Value, Phase.Error, error: SyncWords.SignInError(problem, last.Value, text, p.Finishing));
                return;
            }
            SayError(last switch { SyncFolderKind.Nextcloud => p.NcError, SyncFolderKind.WebDav => p.DavError, _ => p.Error }, text);
            p.Pages.Show(last switch { SyncFolderKind.Nextcloud => "nextcloud", SyncFolderKind.WebDav => "webdav", _ => "list" });
        }
    }
}
