---
title: Adding books
description: Read manga, manhwa and light novels from your own folders, a Komga, Kavita or OPDS server you run, or a book plugin.
---

Kaeri reads manga, manhwa and light novels from folders, servers and book plugins, and the series
it finds show up in Home, Library and Search beside each other.

To add one, open **Your sources** and choose **Add a source**. On a Mac and an iPad it's in the
sidebar, on an iPhone it's in Settings, and on Android it's **Book sources** under Settings ›
Sources. There are three kinds: **A folder**, **A server** and **A plugin store**, which adds book plugins.

## A folder

Pick the folder that holds your books. Each folder directly inside it becomes a series, and a book
lying in it on its own becomes a series of one, named after the file. A folder of images is read
as one book.

Kaeri opens CBZ, CBR and CB7 archives, EPUB and PDF files, and folders of images. MOBI and AZW3
files don't open.

The volume and chapter come from the book's own details first (ComicInfo in an archive, the
package details in an EPUB), then from the file name. Kaeri never writes into the folder. It
reads the folder when you add it and again when you choose **Look again**, or pull down on an
iPhone or iPad, and skips any file it has seen before with the same size and date.

On an iPhone or iPad the folder comes from the Files app, and on Android from the system's folder
picker.

## A server

Kaeri reads from a Komga or Kavita server you run, and from any OPDS catalogue.

- Komga asks for its address, your username and your password.
- Kavita asks for the same, or for its OPDS link instead. The link carries your key, so no
  password is needed.
- An OPDS catalogue asks for its address, and a username and password if it has them. Kaeri reads
  OPDS 1.x catalogues and streams pages from servers that offer page streaming.

**Send progress to the server** is on from the start, and it tells Komga or Kavita which chapters
you've read. An OPDS catalogue gets nothing back.

## A book plugin

Book plugins come from a [plugin store](/docs/plugins/stores/) you've joined, the same way as
plugins for shows. They run in the same [sandbox](/docs/plugins/sandbox/), and each one can reach
only the hosts it lists.

## Tracking

Chapters you finish sync to AniList, MyAnimeList, Kitsu and MangaUpdates, and your place in each
series syncs between your devices. [Trackers and sync](/docs/guides/trackers-sync/) says how to
sign in. The Linux app has no MangaUpdates sign-in yet.
