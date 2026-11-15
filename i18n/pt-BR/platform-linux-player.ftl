party-status = Estado da { -watch-party }
party-members = Membros
party-choose-a-source = Escolha uma { -source }
party-sources-none-listed = Nenhuma { -source } lista o episódio { $episode }. As { -source-pl } vêm de { -plugin-pl }, que você adiciona de uma { -plugin-store }.
party-sources-found-line = { $count ->
    [one] { $count } { -source } tem o episódio. A que você escolher reproduz com a { -watch-party }.
    [many] { $count } { -source-pl } têm o episódio. A que você escolher reproduz com a { -watch-party }.
   *[other] { $count } { -source-pl } têm o episódio. A que você escolher reproduz com a { -watch-party }.
 }
player-could-not-start = O player não iniciou. Tente de novo ou escolha outra { -source }.
player-file-unreadable = Não foi possível ler esse arquivo. Tente outro.
player-inhibit-reason = Reproduzindo um episódio
player-time-left = { $time } restantes
player-note-waited = { $note } · { $seconds } s
player-starting-plain = Iniciando
player-undo-tooltip = Desfazer (U)
player-time-opening = { $clock } · Abertura
player-time-ending = { $clock } · Encerramento
player-channels-mono = Mono
player-channels-stereo = Estéreo
player-channels = { $count ->
    [one] { $count } canal
    [many] { $count } canais
   *[other] { $count } canais
 }
player-translation-label = { -subbed-cap } ou { -dubbed }
player-volume-percent = { $percent } por cento
player-announce-up-next-counting = { -up-next-cap }, { $title }. Começa em { $seconds } segundos. Pressione Esc para cancelar.
player-announce-up-next-waiting = { -up-next-cap }, { $title }. Pressione Enter para reproduzir ou Esc para cancelar.
player-announce-skip = { $label }. Pressione S.
player-announce-undo = { $line }. Pressione U para desfazer.
discussion-like-named = { $count ->
    [one] Curtir, { $count } curtida
    [many] Curtir, { $count } de curtidas
   *[other] Curtir, { $count } curtidas
 }
