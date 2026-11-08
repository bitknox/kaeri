reading-next-chapter = 第{ $chapter }話
reading-usually-mondays = たいてい月曜更新
reading-usually-tuesdays = たいてい火曜更新
reading-usually-wednesdays = たいてい水曜更新
reading-usually-thursdays = たいてい木曜更新
reading-usually-fridays = たいてい金曜更新
reading-usually-saturdays = たいてい土曜更新
reading-usually-sundays = たいてい日曜更新
reading-about-every-weeks = { $weeks ->
   *[other] 約{ $weeks }週間ごと
 }
reading-also-a-show = アニメ版もあります
reading-also-a-book = 原作もあります
reading-split-chapter = 第{ $chapter }話は { -tracker-pl } には { $whole } として送られます。
reader-resumed = { $page }ページから再開しました
reader-resumed-image = { $image }枚目の画像から再開しました
reader-back-to-page = { $page }ページに戻る
reader-moved = { $device }が止まった第{ $chapter }話の{ $page }ページに移動しました
reader-moved-number = { $device }が止まった第{ $chapter }話の{ $page }ページに移動しました
reader-detected-right-to-left = 右から左のようです
reader-detected-left-to-right = 左から右のようです
reader-detected-strip = 縦スクロールのようです
reading-failed-offline = オフラインです。ダウンロード済みの章は開けます。
reading-failed-source-down = { $source }から応答がありません。しばらくしてからもう一度お試しください。
reading-failed-not-found = この{ -source }にはもうありません。別の{ -source }をお試しください。
reading-failed-blocked = { $source }がリクエストを拒否しました。{ $reason }
reading-failed-parse = { $source }から届いた内容を{ -app-name }で読み取れませんでした。その{ -plugin }の更新が必要かもしれません。
reading-failed-page = このページを読み込めませんでした(エラー { $status })。もう一度お試しください。
reading-failed-sign-in = 先を読むには、{ $what }にもう一度サインインしてください。
reading-failed-file-missing = { $path }が見つかりません。ドライブにある場合は、接続してからもう一度お試しください。
reading-failed-unreadable = { $path }を{ $format }として読み取れませんでした。ファイルが壊れている可能性があります。
reading-failed-unsupported = { -app-name }は{ $kind }ファイルを読み取れません。
reading-failed-protected = この本はコピー防止がかかっているため、{ -app-name }では開けません。
reading-failed-disk-full = この{ -device }に空き容量がありません。容量を空けてからもう一度お試しください。
reading-failed-webview = 小説の表示にはバージョン { $version } より新しいシステム WebView が必要です。Android System WebView を更新してからもう一度お試しください。
reading-failed-needs-newer = この{ -plugin }には、より新しい{ -app-name }({ $world })が必要です。使うには{ -app-name }を更新してください。
reading-failed-off = このバージョンの{ -app-name }では読書機能を使えません。
reading-failed-other = 問題が発生しました。{ $reason }
reading-failed-send-too-large = { $file }は{ $size }ですが、{ $target }で扱えるファイルは{ $limit }までです。一度に送る章を減らして、もう一度お試しください。
reading-failed-send-missing = { $file }に{ $part }がないため、送信しませんでした。もう一度お試しください。
reading-failed-send-encoding = { $file }の{ $part }の文字コードが正しくないため、送信しませんでした。もう一度お試しください。
reading-failed-send-malformed = { $file }の{ $part }が無効なため、送信しませんでした。もう一度お試しください。
reading-kind-manga = マンガ
reading-kind-manhwa = マンファ
reading-kind-manhua = マンホア
reading-kind-webtoon = ウェブトゥーン
reading-kind-one-shot = 読み切り
reading-kind-light-novel = ライトノベル
reading-kind-novel = 小説
reading-status-releasing = 連載中
reading-status-finished = 完結
reading-status-hiatus = 休載中
reading-status-cancelled = 打ち切り
reading-status-not-yet = 未公開
reading-chapters = { $count ->
   *[other] { $count }話
 }
reading-volumes = { $count ->
   *[other] { $count }巻
 }
reading-on-planning = 「予定」リストに入っています
reading-spotlight-new-chapter = フォロー中の作品に新しい話があります
reading-spotlight-continue = { -continue-reading-cap }
reading-spotlight-up-next = 次に読む作品
reading-spotlight-popular = 人気の連載中作品
reading-out-today = 第{ $chapter }話が今日公開
reading-out-today-titled = 第{ $chapter }話「{ $title }」が今日公開
reading-chapter-page = 第{ $chapter }話 · { $page }ページ
reading-chapter-percent = 第{ $chapter }話 · { $percent }%
reading-chapter-new = 第{ $chapter }話が新着
reading-next-part = パート{ $part }
reading-part-percent = パート{ $part } · { $percent }%
reading-part-new = パート{ $part }が新着
reading-label-percent = { $label } · { $percent }%
reading-label-new = { $label }が公開されました
reading-part-titled = パート{ $part } · { $title }
reading-label-titled = { $label } · { $title }
reading-part-out-today = パート{ $part }が今日公開
reading-label-out-today = { $label }が今日公開
reading-part-out-today-titled = パート{ $part }「{ $title }」が今日公開
reading-label-out-today-titled = { $label }「{ $title }」が今日公開
reading-chapter-of = 第{ $chapter }話 / 最新 第{ $of }話
reading-volume-part = { $volume }巻 パート{ $part }
reading-volume-label = { $volume }巻、{ $label }
reading-origin-japan = 日本
reading-origin-korea = 韓国
reading-origin-china = 中国
reading-origin-taiwan = 台湾
export-device-other = その他の電子書籍リーダー
export-device-eink-6 = E Ink 6インチ
export-device-eink-7 = E Ink 7インチ
export-device-colour-eink = カラー E Ink 7インチ
export-device-tablet = タブレット
export-chapter = 第{ $chapter }話
export-waits-for-wifi = { $n ->
   *[other] 未ダウンロードの章が{ $n }件あるため、Wi-Fi 接続を待ってから自動で送信します。
 }
reading-chapter-titled = 第{ $chapter }話 · { $title }
reading-row-read = 既読
reading-row-new = 新着
reading-row-downloaded = ダウンロード済み
reading-row-gone-from-source = { -source }から削除済み
reading-row-not-downloaded = 未ダウンロード
reading-row-on-device = この{ -device }内
reading-row-queued = 待機中
reading-row-waits-wifi = 待機中 · Wi-Fi待ち
reading-row-downloading = ダウンロード中 · { $done }/{ $total }ページ
reading-row-failed = ダウンロード失敗
reading-row-page-of = { $page }/{ $of }ページ
reading-row-page = { $page }ページ
reading-row-percent = { $percent }%
reading-row-opened = 読みかけ
reading-row-bookmarked = しおり付き
reading-row-pages = { $n ->
   *[other] { $n }ページ
 }
reading-row-versions = { $n ->
   *[other] { $n }バージョン
 }
reading-ago-now = たった今
reading-ago-minutes = { $minutes }分前
reading-ago-today = 今日
reading-ago-yesterday = 昨日
reading-ago-days = { $days ->
   *[other] { $days }日前
 }
reading-ago-weeks = { $weeks ->
   *[other] { $weeks }週間前
 }
reading-ago-months = { $months ->
   *[other] { $months }か月前
 }
reading-ago-years = { $years ->
   *[other] { $years }年前
 }
reading-removed-from = { $tracker }から削除しました
reading-removed-not-there = { $tracker }のリストにありませんでした
reading-removed-unmatched = { $tracker }で照合できていないため、何も変更していません
reading-removed-failed = { $tracker }のリストに残っています
reading-removed-sign-in = { $tracker }にもう一度サインインしてから、やり直してください。
reading-miss-no-series = この名前の作品はありません
reading-miss-down-since = { $time }から応答なし
reading-miss-down = 応答なし
reading-dl-parts = { $count ->
   *[other] { $count }パート
 }
reading-dl-size = { $size }
reading-dl-read = 既読 { $count }件
reading-dl-chapter = 第{ $chapter }話
reading-dl-pages = { $done }/{ $total }ページ
reading-dl-more-queued = ほか{ $count }件が待機中
reading-dl-queued = { $count }件が待機中
reading-dl-waits-wifi = Wi-Fi待ち
reading-dl-failed = 第{ $chapter }話をダウンロードできませんでした
reading-estimate-size = 約{ $size }
reading-estimate-wifi = Wi-Fi経由
reading-estimate-mobile = モバイルデータ経由
reading-estimate-waits-wifi = Wi-Fi待ち
reading-downloads-done-title = ダウンロードが完了しました
reading-downloads-done-chapters = { $count ->
   *[other] { $title }の{ $count }話をオフラインで読めます。
 }
reading-downloads-done-parts = { $count ->
   *[other] { $title }の{ $count }パートをオフラインで読めます。
 }
reading-downloads-done-series = { $count }作品の章をオフラインで読めます。
read-home-trending-manga = 話題のマンガ
read-home-trending-webtoons = 話題のウェブトゥーン
read-home-trending-novels = 話題のライトノベル
read-home-new-manga = 新作マンガ
read-home-new-webtoons = 新作ウェブトゥーン
read-home-new-novels = 新作ライトノベル
read-home-new-side = 過去12か月
read-home-finished-manga = 完結したマンガ
read-home-finished-webtoons = 完結したウェブトゥーン
read-home-finished-novels = 完結したライトノベル
read-home-finished-side = 人気順
read-search-started-any = 指定なし
read-search-started-year = 過去12か月
read-search-started-3-years = 過去3年
read-hit-chapters = 第1話〜第{ $last }話
read-hit-episodes = 第1話〜第{ $last }話
read-hit-parts = { $count ->
   *[other] { $count }部
 }
read-hit-files = { $count ->
   *[other] ファイル{ $count }件
 }
read-hit-files-format = { $count ->
   *[other] ファイル{ $count }件 · { $format }
 }
read-source-folders = フォルダ
reading-since-january = 1月から
reading-since-february = 2月から
reading-since-march = 3月から
reading-since-april = 4月から
reading-since-may = 5月から
reading-since-june = 6月から
reading-since-july = 7月から
reading-since-august = 8月から
reading-since-september = 9月から
reading-since-october = 10月から
reading-since-november = 11月から
reading-since-december = 12月から
