source-picker-seeders-linux = { $count ->
    [one] Um semeador
    [many] { $count } semeadores
   *[other] { $count } semeadores
 }
sidebar-show = Mostrar barra lateral
activity-calendar = Carregando a programação
activity-episodes = Carregando episódios
activity-sources = Verificando { -source-pl }
activity-downloads = Carregando downloads
activity-browse = Carregando títulos
size-per-second = { $size }/s
time-now = Agora
time-just-now = Agora mesmo
spoken-hours = { $count ->
    [one] { $count } hora
    [many] { $count } horas
   *[other] { $count } horas
 }
spoken-minutes = { $count ->
    [one] { $count } minuto
    [many] { $count } minutos
   *[other] { $count } minutos
 }
spoken-seconds = { $count ->
    [one] { $count } segundo
    [many] { $count } segundos
   *[other] { $count } segundos
 }
continue-row-caught-up-airs = Em dia · E{ $episode } vai ao ar { $when }
continue-row-caught-up-not-out = Em dia · E{ $episode } ainda não saiu
language-name-english = Inglês
language-name-japanese = Japonês
language-name-spanish = Espanhol
language-name-portuguese = Português
language-name-french = Francês
language-name-german = Alemão
language-name-italian = Italiano
language-name-russian = Russo
language-name-arabic = Árabe
language-name-indonesian = Indonésio
language-name-korean = Coreano
language-name-chinese = Chinês
seasonal-format-all = Todos
seasonal-format-ona-ova = ONA e OVA
title-open-named = Abrir { $title }
continue-row-spoken = { $title }, { $detail }
continue-preview = Prévia
up-next-airs-at = Vai ao ar às { $time }
progress-name = Progresso
progress-percent = { $percent } por cento
poster-grid-name = Títulos
onboarding-where-hosts-linux = O { -app-name } reproduz o que você já tem: arquivos neste { -device }, seu servidor Jellyfin e { -plugin-pl } de { -plugin-store-pl } em que você entrar.
sources-empty-or = Ou
library-row-summary-linux = { $episodes ->
    [one] 1 episódio de { $series } séries
    [many] { $episodes } de episódios de { $series } séries
   *[other] { $episodes } episódios de { $series } séries
 }
library-page-watching-linux = Alterações são acompanhadas
library-page-no-watch-linux = Mudanças verificadas quando o { -app-name } inicia
library-matched-linux = Identificadas
library-best-guess-linux = Melhor palpite: { $title }
library-fix-count-mismatch-linux = { $episodes ->
    [one] Tem 1 episódio, esta pasta tem { $files }
    [many] Tem { $episodes } de episódios, esta pasta tem { $files }
   *[other] Tem { $episodes } episódios, esta pasta tem { $files }
 }
