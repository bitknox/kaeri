servers-add-title = Adicionar um servidor de mídia
servers-on-network = Na sua rede
servers-looking = Procurando servidores…
servers-none-found = Nenhum servidor encontrado nesta rede. Digite o endereço.
servers-address = Endereço do servidor
servers-address-hint = jellyfin.home.example ou 192.168.1.20:8096
servers-address-note = Digite o endereço do seu servidor, o mesmo que você usa para abrir o Jellyfin no navegador.
servers-continue = { -proceed-cap }
servers-sign-in-title = Entrar em { $server }
servers-user = Nome de usuário
servers-password = Senha
servers-use-quick-connect = Usar o Quick Connect
servers-use-password = Entrar com senha
servers-token-note = Sua senha não é salva. O { -app-name } guarda um token de acesso em { $store }.
secure-store-windows = Gerenciador de Credenciais
secure-store-linux = seu chaveiro
secure-store-device = seu Chaveiro
secure-store-android = o armazenamento seguro deste { -device }
servers-sign-in = Entrar
servers-qc-title = Digite este código no Jellyfin
servers-qc-steps = Em um { -device } já conectado, abra o Jellyfin, vá ao seu perfil, escolha Quick Connect e digite este código.
servers-qc-waiting = Aguardando a sua aprovação
servers-qc-expired = Esse código expirou.
servers-qc-new-code = Gerar novo código
servers-libraries-title = Escolha as bibliotecas
servers-libraries-body = As séries são identificadas no AniList para entrar no Início, nas suas listas e na Busca.
servers-library-line = { $kind } · { $count } séries
servers-report = Informar ao Jellyfin o que você assiste
servers-report-line = Pontos de retomada e marcas de assistido acompanham o que você reproduz aqui.
servers-add = Adicionar
servers-row-matching = Identificando · { $done } de { $total } séries
servers-row-summary = { $libraries ->
    [one] 1 { -library } · { $series } séries
    [many] { $libraries } de bibliotecas · { $series } séries
   *[other] { $libraries } bibliotecas · { $series } séries
 }
servers-row-libraries = { $count ->
    [one] 1 { -library }
    [many] { $count } de bibliotecas
   *[other] { $count } bibliotecas
 }
servers-row-series = { $count ->
    [one] 1 série
    [many] { $count } de séries
   *[other] { $count } séries
 }
servers-row-needs-look = { $count } precisam de revisão
servers-page-line = { $address } · Jellyfin { $version } · conectado como { $user }
servers-sign-out = Sair
servers-remove = Remover servidor
servers-error-unreachable = Não foi possível acessar { $address }. Confira o endereço e se o servidor está ligado.
servers-error-not-jellyfin = Esse endereço responde, mas não é um servidor Jellyfin.
servers-libraries = Bibliotecas
servers-remove-line = Nada é apagado no servidor.
servers-removed = { $name } removido
servers-error-too-old = Este servidor usa o Jellyfin { $version }. O { -app-name } precisa da 10.8 ou mais recente.
servers-error-credentials = Nome de usuário ou senha incorretos.
servers-error-qc-off = O Quick Connect está desativado neste servidor. Entre com senha ou ative-o no painel do Jellyfin.
servers-error-plain-http = Este endereço não está na sua rede e não usa HTTPS, então sua senha viajaria sem proteção. Use um endereço https://.
servers-error-bad-address = Isso não parece um endereço. Digite-o como você abre o Jellyfin no navegador.
servers-error-other = Algo deu errado. Tente de novo.
servers-cert-title = Verifique este servidor
servers-cert-body = { $host } usa um certificado em que este { -device } não confia. Isso é normal para um servidor em casa. Permita só se esta impressão digital for igual à que o seu servidor mostra.
servers-cert-allow = Permitir este certificado
servers-row-signed-out = Desconectado pelo servidor
servers-sign-in-again = Entrar de novo
servers-row-not-answering = Sem resposta · visto por último { $time }
title-server-not-answering = { $server } não está respondendo. Os episódios que você baixou continuam reproduzindo.
servers-kind-shows = Séries
servers-kind-movies = Filmes
servers-kind-other = Vídeos
servers-quick-connect = Quick Connect
servers-libraries-heading = Bibliotecas
servers-refresh = Identificar de novo
servers-needs-look = { $count } não identificadas no AniList
servers-kind-films = Filmes
servers-error-certificate = { $host } usa um certificado em que este { -device } não confia. Use um endereço https:// com um certificado em que ele confie.
servers-row-signed-out-here = Desconectado
servers-sign-out-line = Encerra o acesso deste { -device } ao servidor. O servidor e seus downloads continuam como estão.
