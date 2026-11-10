## Windows: the series page's notices, Send to e-reader's saved files, the OPDS feed's Windows wording and the firewall question before the feed first starts.

# Notice after Read again from the start, with an Undo button beside it. The title is the series' name.
# @only windows
# @arg title: text
# @max 64
series-read-again-done-windows = Reading { $title } again from the start

# Button in Send to e-reader, once the files are ready: saves them where the person picks instead of sharing them.
# @only windows
# @max 24
send-save-files-windows = Save files

# Notice after the files for an e-reader were saved.
# @only windows
# @arg n: int
# @max 48
send-saved-windows = { $n ->
    [one] Saved the file
   *[other] Saved { $n } files
}

# Notice when the files for an e-reader could not be saved where the person picked.
# @only windows
# @max 72
send-save-failed-windows = Couldn't save the files there. Try another folder.

# Shown in place of the feed's address while the feed is on but this computer has no local network.
# @only windows
# @max 64
opds-paused-none-windows = Paused. This PC isn't on a local network.

# Shown in place of the feed's address while the feed is on but the only connection goes through a VPN.
# @only windows
# @max 64
opds-paused-vpn-windows = Paused. This PC is only connected through a VPN.

# Title of the dialog shown once, the first time the OPDS feed is switched on, before Windows asks whether Kaeri may use the network.
# @only windows
# @max 48
feed-firewall-title-windows = Windows will ask about the network

# Body of that dialog. "Private networks" and "public networks" are the names Windows' own question uses.
# @only windows
# @max 160
feed-firewall-body-windows = To let e-readers at home reach the feed, allow { -app-name } on private networks only. Leave public networks unticked.

# Header of the group picker that opens from the Group row's Change on a series page. The source is a site or server name, such as "books.home.example".
# @only windows
# @arg source: text
# @max 48
windows-group-picker-in = Group in { $source }
