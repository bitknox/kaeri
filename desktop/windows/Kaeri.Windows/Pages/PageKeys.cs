namespace Kaeri.Windows.Pages;

/// <summary>The window's Ctrl+Shift+M and Delete, for the page that has something selected.</summary>
interface IPageKeys
{
    bool MarkWatched();
    bool Delete() => false;
}
