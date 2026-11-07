book-server-address = アドレス
book-server-address-hint = アドレス、またはKavitaのOPDSリンク全体を入力します。
book-server-kavita-key = これはKavitaのOPDSリンクです。キーでサインインするため、パスワードは不要です。
book-server-key-kept = { -app-name }はキーを{ $store }に保存し、末尾4文字だけを表示します。
book-server-key = キー ••••{ $last }
book-server-found-title = { $libraries ->
   *[other] { $libraries }件のライブラリで{ $series }シリーズが見つかりました
 }
book-server-found-line = { $host }の{ $kind }です。追加するライブラリを選んでください。
book-server-library-line = { $series }シリーズ
book-server-sync-progress = サーバーに進捗を送る
book-server-sync-progress-line = ここで読んだページは{ $host }に送られ、{ $host }の進捗がここに反映されます。
book-server-username = ユーザー名
book-server-password = パスワード
book-server-err-unreachable = { $host }に接続できませんでした
book-server-err-unreachable-line = アドレスと、サーバーが起動していてこのネットワーク上にあるかを確認してください。
book-server-err-password = { $host }はそのパスワードを受け付けませんでした
book-server-err-password-line = ブラウザーで{ $kind }に使っているユーザー名とパスワードを入力してください。
book-server-err-key = { $host }はそのキーを受け付けませんでした
book-server-err-key-line = キーはKavitaでリセットすると変わります。Kavitaのアカウント設定からOPDSリンクをもう一度コピーしてください。
book-server-empty = { $host }には、このアカウントで表示できるシリーズがありません
book-server-empty-line = サーバーには入れますが、サーバーがまだスキャン中か、このアカウントに本へのアクセス権がない可能性があります。
book-server-empty-recheck = 今すぐ追加できます。{ -app-name }は30分ごとに再確認します。
book-server-not-books-line = { -app-name }は、Komga、Kavita、OPDSのカタログから読み込みます。
book-folder-found = { $files ->
   *[other] { $files }個のファイルに{ $series }シリーズが見つかりました
 }
book-folder-found-line = { $path }。{ -app-name }は、そのうち{ $matched }シリーズを詳細情報で照合しました。
book-folder-unsupported = { $count ->
   *[other] { -app-name }で開けないファイルが{ $count }個
 }
book-folder-unsupported-line = { $kinds }。これらはフォルダー内にそのまま残ります。
book-folder-drm = DRMで保護されたEPUBが{ $count }個
book-folder-opens = { -app-name }は、CBZ、CBR、EPUB、PDFファイルと、画像のフォルダーを開けます。
book-folder-show-files = ファイルを表示
book-folder-add = フォルダーを追加
book-folder-change = 変更
book-folder-empty = { $folder }に{ -app-name }で開けるものはありません
book-folder-empty-line = { $files ->
   *[other] { $files }個のファイルがありますが、CBZ、CBR、EPUB、PDF、画像のいずれでもありません。
 }
book-folder-choose-other = 別のフォルダーを選ぶ
store-join-title = { -plugin-store }に参加
store-invite-field = 招待
store-joined-title = { $store }に参加しました
store-joined-line = { $count ->
   *[other] { $owner }が{ -plugin }を{ $count }個提供しています。参加してもインストールはされません。ここで、または後で選んでインストールできます。
 }
store-group-reading = 読書用
store-group-watching = 視聴用
store-install = インストール
store-installed-row = インストール済み
store-install-all-reading = 読書用をすべてインストール
store-installed = { $count ->
   *[other] { -plugin }を{ $count }個インストールしました。{ -source-pl }に入っています。
 }
store-other-medium = { $store }の{ -plugin }はすべて視聴用です
store-other-medium-line = この{ -device }では視聴がオフなので、{ -source-pl }には表示されません。
store-turn-on-watching = 視聴をオンにする
store-empty = { $store }にはまだ{ -plugin-pl }がありません
store-empty-line = メンバーなので、{ $owner }が追加すると、新しい{ -plugin-pl }が{ -plugin-store-cap-pl }に表示されます。
store-err-unreachable = { $host }に接続できませんでした
store-err-unreachable-line = 招待は正しいようですが、{ -store }が応答していません。しばらくしてからもう一度試すか、稼働しているか{ $owner }に確認してください。
store-other-medium-more = { $count ->
   *[other] ほかに{ $count }個は視聴用ですが、この{ -device }では視聴がオフです。
 }
store-tv-reading-only = { $count ->
   *[other] ほかに{ $count }個は読書用ですが、このTVでは読書できません。
 }
store-tv-all-reading = { $store }の{ -plugin }はすべて読書用ですが、このTVでは読書できません。
store-plugin-row = バージョン{ $version } · { $hosts }に接続する場合があります
source-added = { $name }を追加しました · { $series }シリーズ
source-added-plugins = { $count ->
   *[other] { $store }から{ -plugin }を{ $count }個インストールしました
 }
source-row-server = { $libraries ->
   *[other] { $kind } · { $libraries }件のライブラリに{ $series }シリーズ · 追加したばかり
 }
source-row-server-empty = { $kind } · シリーズはまだありません · 30分後に再確認します
source-row-folder = { $skipped ->
   *[other] { $path } · { $series }シリーズ · { $skipped }個のファイルをスキップ
 }
source-row-plugin = { $store }から · { $medium } · バージョン{ $version }
