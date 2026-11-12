---
title: E-ink and e-readers
description: Kaeri on an Android e-ink tablet, Send to e-reader, and the OPDS feed for e-readers at home.
---

## On an e-ink tablet

The Android app knows most e-ink tablets by their maker, by the system's own e-ink setting or by a
slow screen. When it can't tell, it asks **Is this an e-ink screen?**, with **Use the e-ink
layout** or **Keep the standard layout**. When it can't tell whether the screen shows colour, it
asks **Does this screen show colour?**, with **Yes, colour**, **Only greys** or **Skip**.

The e-ink layout turns pages instead of scrolling, and nothing in it animates. The reader starts in
the **Paper** look, with the page borders cropped and the volume keys turning pages. To clear the
ghosts e-ink leaves behind, Kaeri refreshes the whole screen: every 6 pages of text, every 4 dark
pages, every 5 screens of a list, and on every page of a manga. **Clean screen** in the reader does it
at once.

The same APK runs on phones, tablets and e-ink tablets with Android 11 or later.

## Send to e-reader

**Send to e-reader** is in a chapter's menu, and **E-readers** in Settings keeps the e-readers you
send to. Each one has a kind: Kobo, Kindle, Other e-reader, a 6 or 7 inch e-ink screen, a 7 inch
colour e-ink screen, or a tablet. The kind picks the file. A Kobo gets KEPUB. The others get a
fixed-layout EPUB or a CBZ for manga, and an EPUB for light novels.

Chapters you haven't downloaded are fetched for the file, waiting for Wi-Fi if your downloads do,
and removed again afterwards. **Prepare and share** hands the file to the system's share sheet, and
on Windows **Save files** saves it to a folder.

## The OPDS feed

The Mac and Windows apps can serve your books to e-readers on your home network as an OPDS feed.
It's off until you turn on **Share on this network** under **OPDS feed** in Settings.

- It never runs without a password. The password stays the same until you choose **New
  password**, and any username works.
- **Port** can stay empty, which picks a free port, or be any port from 1024 to 65535.
- The feed's address looks like `http://192.168.1.20:8080/opds`.
- It shares your downloaded chapters and your manga and novel folders.
- It answers only devices on your local network and runs only while Kaeri is open. It pauses when
  the computer has no local network, or only a VPN.

On Linux the feed has its switch, port and password in Settings. The rest of it is in development.
