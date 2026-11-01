queue-episodes-left = { $count ->
   *[other] { $title } · 残り { $count }話
 }
episodes-count = { $count ->
   *[other] { $count }話
 }
progress-episodes = { $total ->
   *[other] { $watched }/{ $total }話
 }
duration-days-hours = { $days }日{ $hours }時間
group-in-progress = 視聴中
group-new-this-week = 今週の新着
continue-empty-title = まだ視聴した作品はありません
continue-load-failed = { -continue-cap }を読み込めませんでした
continue-airing-today = 今日放送
continue-this-season = 今シーズン
continue-hide = { -continue-cap }から非表示
continue-hidden = { -continue-cap }から非表示にしました。
continue-drop = ドロップに移動
continue-more-for = { $title }のその他の操作
continue-row-resume = E{ $episode }を再開 · { $clock }
continue-row-resume-dubbed = E{ $episode } { -dubbed }を再開 · { $clock }
continue-row-start = E{ $episode }を再生
continue-row-start-dubbed = E{ $episode } { -dubbed }を再生
continue-row-start-aired = E{ $episode }を再生 · { $ago }に公開
continue-row-caught-up = E{ $episode }まで視聴済み · { $when }
continue-row-caught-up-plain = 視聴済み
continue-widget-connect = AniListに接続すると、ここに表示されます。
continue-caught-up = すべて視聴済みです。
title-button-resume = 第{ $episode }話を再開 · { $time }
title-button-play = 第{ $episode }話を再生
title-button-add-source = { -source }を追加して再生
library-status-watching = 視聴中
library-status-planning = 視聴予定
library-status-completed = 視聴完了
library-status-paused = 保留中
library-status-dropped = ドロップ
library-sort-updated = 最終更新
library-sort-title = タイトル
library-sort-score = 評価
library-sort-button = 並び替え: { $sort }
library-sort-label = { $sort }で並び替え
library-signed-out-title = リストはAniListから取得します
library-signed-out-detail = サインインすると、視聴状況と中断した位置を同期できます。
library-signed-out-action = AniListにサインイン
library-list-empty = 「{ $list }」リストは空です
library-list-empty-detail = AniListでこのリストに追加した作品が、ここに表示されます。
library-list-load-failed = 「{ $list }」リストを読み込めませんでした
library-loading = リストを読み込んでいます
library-add-to-list = リストに追加
library-add-planning = 視聴予定に追加
library-added-planning = 視聴予定に追加しました。
library-on-your-list = リストに登録済み
library-in-your-list = 登録済み
progress-episodes-short = E{ $done }/{ $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = E{ $episode } { -dubbed }を再生 · { $ago }に公開
up-next-starts = { $date }放送開始
up-next-reason-new-season = 新シーズン
up-next-reason-planning = 視聴予定リストにあります
up-next-reason-recommended = { $title }を見終えたため
up-next-reason-recommended-plain = おすすめ
up-next-because-detail = { $title }を好きな人たちがAniListでおすすめしている作品
up-next-add-planning = 視聴予定に追加
up-next-not-interested = 興味なし
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title }を{ -up-next-cap }のおすすめから非表示にしました。
up-next-hidden-until-airs = { $title }は放送が始まるまで非表示になります。
up-next-broadcast = 第{ $episode }話 · { $when }
up-next-out = { $ago }に公開
library-row-scanning = { $files ->
   *[other] スキャン中 · { $files }ファイルを検出
 }
library-row-matching = 照合中 · { $done } / { $total }シリーズ
library-row-waiting = AniListを待っています · { $seconds }秒
library-row-summary = { $series }、{ $episodes }
library-episodes = { $count ->
   *[other] { $count }エピソード
 }
library-series = { $count ->
   *[other] { $count }シリーズ
 }
library-row-needs-look = { $count }件は要確認
library-row-drive-offline = ドライブが接続されていません
library-row-missing = フォルダーが見つかりません
library-row-no-access = { -app-name }はこのフォルダーを開けなくなりました
library-locate = 場所を指定…
library-choose-again = 選び直す
library-page-watching = 変更を監視中 · 最終スキャン { $time }
library-page-no-watch = { -app-name }の起動時に再スキャン · 最終スキャン { $time }
library-rescan = 再スキャン
library-show-explorer = エクスプローラーで表示
library-remove-folder = フォルダーを削除
library-needs-look = 要確認
library-not-matched = 未照合
library-matched = { $count }シリーズを照合済み
library-best-guess = 推定:{ $title } · { $percent }%一致
library-picker-prompt = フォルダーを追加
library-picker-message = エピソードが入ったフォルダーを選んでください。
library-fix-confidence = { $percent }%一致
library-open-folder = フォルダーを開く
library-matched-count = { $count }シリーズを照合済み
library-row-series = { $count ->
   *[other] { $count }シリーズ
 }
library-row-files = { $files ->
   *[other] { $files }ファイル
 }
library-show-finder = Finderで表示
library-remove-line = ファイルは元の場所に残ります。
library-files = { $count ->
   *[other] { $count }ファイル
 }
library-reason-no-episode = 名前にエピソード番号がありません
library-reason-no-title = AniListで作品が見つかりません
library-reason-not-video = { -app-name }で再生できる動画ファイルではありません
library-set-episode = 話数を設定…
library-keep-unmatched = 未照合のままにする
library-fix-title = このフォルダーはどの作品ですか?
library-reason-no-result = AniListで作品が見つかりません
library-kept = そのままにしています
library-fix-search = AniListを検索
library-fix-hint = これらのファイルを照合する作品を検索してください。
library-connect-drive = ドライブを接続して再生
library-fix-count-mismatch = { $titleEpisodes }エピソード、このフォルダーは{ $files }ファイル
library-leave-unmatched = 未照合のままにする
library-episode-title = このファイルはどのエピソードですか?
library-season = シーズン
library-episode = エピソード
library-apply = 適用
library-from-files = お手元のファイルから
title-connect-drive = ドライブを接続して再生
library-picker-title = エピソードが入ったフォルダーを選んでください。
library-remove-detail = ファイルは元の場所に残ります。
library-reason-offline = AniListに問い合わせられませんでした。次回のスキャンで再試行します。
library-choose-match = 照合先を選ぶ
library-kept-line = 独立した作品として保持
library-fix-match = この照合を修正
library-fix-none = 作品が見つかりません
library-fix-offline = AniListに接続できませんでした。オンラインになってからもう一度お試しください。
library-removed = { $name }を削除しました
library-match-percent = { $percent }%一致
