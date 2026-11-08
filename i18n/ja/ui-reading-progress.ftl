progress-server-saved = { $server }に保存しました
progress-server-sending = { $server }に送信中…
progress-server-offline = オンラインに戻ると{ $server }に送信します
progress-server-retrying = { $server }から応答がありません。{ -app-name }が再試行します。
progress-server-failed = { $server }に保存できませんでした：{ $reason }。
progress-why-key = サーバーがキーを受け付けませんでした
progress-why-password = サーバーがパスワードを受け付けませんでした
progress-why-gone = この作品はもうサーバーにありません
progress-why-gave-up = 1日応答がありませんでした
progress-row-saved = { $ago }に進捗を保存しました
progress-row-sending = { $count ->
   *[other] { $count }話を送信中…
 }
progress-row-offline = { $count ->
   *[other] { $count }話はオンラインに戻ると送信されます
 }
progress-row-retrying = { $count ->
   *[other] { $count }話が待機中です。{ -app-name }が再試行します。
 }
progress-row-failed = { $count ->
   *[other] { $count }話を保存できませんでした：{ $reason }
 }
progress-row-off = 進捗はこの{ $kind }に残ります
