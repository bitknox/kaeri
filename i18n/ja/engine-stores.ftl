store-joined = { $store }に{ $member }として参加しました。
store-joined-unnamed = { $store }に名前なしで参加しました。
store-left = { $store }から退出しました。インストール済みの{ -plugin-pl }はそのまま残ります。
plugin-removed = { $plugin }を削除しました。
store-member-removed = 削除しました。このメンバーの{ -device-pl }は、この{ -plugin-store }で使えなくなりました。
store-invite-cancelled = 招待を取り消しました。このリンクは使えなくなりました。
store-invite-cancelled-kept = { $count ->
   *[other] 招待を取り消しました。参加した{ $count }人はメンバーのままです。
 }
store-open-join-on = { -plugin-store }のリンクを知っている人は、今から参加できます。メンバーの画面からリンクをコピーできます。
store-open-join-off = 今から参加できるのは、招待された人だけです。
store-open-join-off-devices = { $devices ->
   *[other] 今から参加できるのは、招待された人だけです。リンクから参加した{ $devices }台の{ -device-pl }は、削除するまで引き続き使えます。
 }
store-device-removed = この{ -device }は、この{ -plugin-store }で使えなくなりました。
store-devices-removed = リンクから参加したすべての{ -device }を削除しました。
plugin-published = { $plugin } { $version }を公開しました。
plugin-unpublished = { $plugin }は配布されなくなりました。
store-admin-opened = ブラウザで管理ページを開きました。このリンクは一度だけ使え、10分間有効です。
store-admin-open-failed = ブラウザを開けませんでした（{ $reason }）。{ -plugin-store }のマシンで「kaeri store admin」を実行すると、リンクが表示されます。
plugin-installed-unused = { $plugin } { $version }をインストールしましたが、{ -source }の順序で{ -plugin-pl }がオフになっているため、使われません。
plugin-installed-name-clash = { $plugin }をインストールしましたが、{ $id }は組み込みの{ -source }の名前のため、読み込まれませんでした。
plugin-installed = { $plugin } { $version }をインストールしました。
plugin-installed-not-loaded = { $plugin }をインストールしましたが、読み込めませんでした。{ $reason }
plugin-installed-outdated = { $plugin }をインストールしましたが、{ -app-name }の古いバージョン向けに作られているため、更新が必要です。作者に新しいビルドを依頼してください。
plugins-outdated = { $count ->
   *[other] { $count }件の{ -plugin-pl }が{ -app-name }の古いバージョン向けに作られているため、更新が必要です:{ $names }。
 }
plugin-updates-ready = { $count ->
   *[other] { -plugin-cap }の更新があります：{ $names }。
 }
plugins-load-failed = { -plugin-pl }を読み込めませんでした。{ $reason }
store-token-missing = この{ -device }には{ $store }のトークンがありません。新しい招待で、もう一度参加してください。
store-not-joined = その{ -plugin-store }には、もう参加していません。
store-key-damaged = この{ -device }に保存されている、この{ -plugin-store }の鍵が破損しています。{ -plugin-store }から退出し、新しい招待で参加し直してください。
store-device-refused = { $store }は、この{ -device }をもう受け付けていません。新しい招待で、もう一度参加してください。
store-unreachable = { $store }に接続できませんでした。{ $reason }
store-moved = { $store }は{ $current }に参加していますが、このリンクは{ $target }を指しています。{ -plugin-store }の場所が変わった場合は、先に退出してから参加し直してください。
store-join-would-unname = { $store }には{ $member }として参加済みです。このリンクで参加すると名前が外れるため、何も変更しませんでした。
store-not-the-invited = { $url }は、招待に書かれた{ -plugin-store }ではありません（{ $reason }）。参加は行われませんでした。
store-token-not-kept = この{ -plugin-store }のトークンを保存できませんでした。{ $reason }
store-invite-unusable = その招待はすでに使われたか、取り消されたか、期限切れです。新しい招待を依頼してください。
store-invite-used-up = この招待は使用上限に達しました。{ $owners }に新しい招待を依頼してください。
store-invite-ended = この招待は{ $when }に終了しました。{ $owners }に新しい招待を依頼してください。
store-invite-was-cancelled = この招待は取り消されました。{ $owners }に新しい招待を依頼してください。
store-invite-contact = 連絡先:{ $contact }
store-open-link-unusable = このリンクでは、もう誰も参加できません。{ -plugin-store }の所有者に、新しいリンクか招待を依頼してください。
plugin-reserved-name = { $plugin }は組み込みの{ -source }の名前のため、{ -plugin }にはできません。
store-plugin-withdrawn = { $store }は、もう{ $plugin }を配布していません。
store-plugin-changed = { $store }は、表示された後に{ $plugin }を変更しました。インストールする前に、もう一度確認してください。
plugin-from-other-store = { $plugin }は{ $store }からインストールされており、そちらで更新されます。
plugin-duplicate-file = { $path }にはすでに{ $plugin }が宣言されています。先にそのファイルを削除してください。そのままだと、2つのコピーが動いてしまいます。
plugin-install-failed = { $plugin }をインストールできませんでした。{ $reason }
plugin-publish-failed = { $plugin }を公開できませんでした。{ $reason }
plugin-missing-locally = { $plugin }は{ -plugin }フォルダーにもうありません。
store-invite-needs-link = この{ -device }は、リンクで{ $store }に参加しています。もう一方の{ -device }には、同じリンクを貼り付けてください。
store-link-malformed = { -plugin-store }から送られたリンクが正しく動作しません。所有者に{ -plugin-store }を確認してもらってください。{ $reason }
store-link-wrong-key = { -plugin-store }が別の鍵でリンクを作成しました。リンクを開いたときのアドレスを確認してください。
store-admin-missing = { $store }には管理ページがありません。サーバーが古い可能性があります。
store-admin-link-foreign = { $store }から、別のアドレスを指す管理リンクが送られました。
store-open-join-unsupported = { $store }のサーバーは古く、リンクで誰でも参加できるようにはできません。先に更新してください。
source-switched-off = { $name }をオフにしました。エピソードを探すときは飛ばされます。
source-switched-on = { $name }をオンにしました。
plugin-not-loaded = { $id }は読み込まれていません。{ -source-cap-pl }の下にインストールされているか確認してください。
release-pinned = このタイトルには{ $name }を使います。
notice-choice-not-saved = その選択を保存できませんでした。{ $reason }
torrent-check-failed-running = 接続の確認に失敗しました。トレントは動き続けています。
torrent-vpn-waiting = VPNガードを通過すると、トレントが開始されます。{ $reason }
torrent-vpn-unverified = VPNガードを通過すると、トレントが開始されます。接続はまだ確認されていません。
torrent-link-unusable = マグネットリンクでもトレントファイルでもありません。
torrent-link-playback-off = 自分のリンクを再生するには、設定でトレント再生をオンにしてください。
opened-search-failed = その作品を検索できませんでした。{ $reason }
torrent-interface-closed = トレントが開始される前に{ -app-name }が終了しました。
torrent-session-failed = トレントのセッションを開始できませんでした。{ $reason }
torrent-no-kill-switch = VPNが切断された場合にトレントの通信を止める仕組みが、この{ -device }にはありません。「kaeri --doctor」を実行すると設定できます。
torrent-not-checked = 接続を確認できませんでした。{ $reason }
sources-reloaded-torrents-pending = { -source-cap-pl }を再読み込みしました。VPNガードを通過すると、トレントが開始されます。
sources-reloaded-torrents-off = トレントはオフです。セッションを停止しました。
job-anilist-limit = AniListのレート制限を待機しています
notice-anilist-limit = AniListのレート制限のため、{ $wait }待機しています。
notice-anilist-unreachable = AniListが応答していません。当面は、別のサービスのデータを表示しています。
notice-metadata-cached = どのメタデータサービスも応答しませんでした。最後にキャッシュされた内容を表示しています。
notice-anilist-back = AniListに再接続しました。
notice-party-not-built = このアプリには{ -watch-party-cap-pl }が含まれていません。
sources-demo-removed = デモを削除しました
stores-ready-demo-stays = { $store }の準備ができました。デモは「あなたの{ -source-pl }」の最後に残ります。
