---
title: Plugin stores
description: Run a store for your own plugins and your own devices, with a signed list and a token for each device.
---

A plugin store is a small server for your own plugins and your own devices. Every device that
joins can install the store's plugins from inside Kaeri and hears about new versions when
Kaeri starts. Kaeri ships no store and lists none. You join one through an invite link. The store's owner
chooses its plugins and is responsible for them: Kaeri did not make, review or approve them.

## Joining a store

The store's owner sends you an invite link. In Kaeri, open **Sources** and press Enter to
open the plugin stores, choose **Join a store**, and paste the link. Kaeri checks the link against
the store before using it, shows the store's name and key, and asks you to confirm. The
confirmation also shows the name the store's owner will see for this device, which starts as the
kind of device it is, such as "Computer" or "iPhone". Type over it before you join to change it.

The store's page lists its plugins. Enter on one shows what it may contact before anything is
installed. Installed plugins run as part of the `"plugins"` entry in `providers.order`, which is
there by default; `K` on the Sources screen moves one ahead of the others.

An invite link works once and expires after seven days. To use the same store on another computer,
choose **Link another device** on the store's page for a new link of your own.

## Updates

When a store has a newer version of a plugin you installed, Kaeri says so once when it starts.
Nothing is installed until you choose **Update** on the store's page, and an update that wants to
contact a host the old version could not reach says which before you accept it.

## Leaving a store

Choose **Leave this store** on the store’s page to give up your access. Plugins you installed from it stay
on your machine and stop updating. To remove one, choose it on the store's page and pick
**Remove** before you leave.

## Security

Only people with an invite can list or download a store's plugins. Everyone who joins does get the
plugins themselves, and can read them like any other file on their machine.

Members are protected by the sandbox and by the store's signature. A plugin can only reach the hosts
it declares, and a plugin list or download that was not signed by the key in your invite is refused,
as is a list older than one you have already seen. The key lives with the store, so the signature
guards against tampering on the way to you and against another server answering at the store's
address. It does not guard against someone who controls the store itself.

Removing a device stops its updates at once; plugins it already installed stay on that machine.

## Running a store

Anyone can run one, on their own computer or on a hosting platform, and most platforms' free or
smallest plans are enough for a store of your own. [Running a store](/docs/plugins/running-a-store/)
covers deploying one, claiming it, and managing it from Kaeri or its admin page.

The protocol is described in full in
[docs/plugin-stores.md](https://github.com/bitknox/kaeri/blob/main/docs/plugin-stores.md), for
anyone who wants to write a different server.
