book-server-address = Endereço
book-server-address-hint = Um endereço, ou o link OPDS completo do Kavita.
book-server-kavita-key = Este é um link OPDS do Kavita. A chave dele faz seu login, então não precisa de senha.
book-server-key-kept = O { -app-name } guarda a chave em { $store } e mostra só os últimos quatro caracteres dela.
book-server-key = Chave ••••{ $last }
book-server-found-title = { $libraries ->
    [one] Foram encontradas { $series } séries
    [many] Foram encontradas { $series } séries em { $libraries } de bibliotecas
   *[other] Foram encontradas { $series } séries em { $libraries } bibliotecas
 }
book-server-found-line = { $kind } em { $host }. Escolha as bibliotecas a adicionar.
book-server-library-line = { $series } séries
book-server-sync-progress = Enviar o progresso ao servidor
book-server-sync-progress-line = As páginas que você lê aqui voltam para { $host }, e o progresso de lá vem para cá.
book-server-username = Usuário
book-server-password = Senha
book-server-err-unreachable = Não foi possível acessar { $host }
book-server-err-unreachable-line = Confira o endereço e se o servidor está ligado e nesta rede.
book-server-err-password = { $host } não aceitou essa senha
book-server-err-password-line = Use o usuário e a senha que você usa para { $kind } em um navegador.
book-server-err-key = { $host } não aceitou essa chave
book-server-err-key-line = A chave muda quando é redefinida no Kavita. Copie o link OPDS de novo nas configurações da sua conta lá.
book-server-empty = { $host } não mostra séries para esta conta
book-server-empty-line = Ele deixa você entrar, mas o servidor pode ainda estar verificando, ou esta conta pode não ter acesso aos livros dele.
book-server-empty-recheck = Você pode adicioná-lo agora. O { -app-name } confere de novo a cada 30 minutos.
book-server-not-books-line = O { -app-name } lê catálogos Komga, Kavita e OPDS.
book-folder-found = { $files ->
    [one] Foram encontradas { $series } séries em um arquivo
    [many] Foram encontradas { $series } séries em { $files } de arquivos
   *[other] Foram encontradas { $series } séries em { $files } arquivos
 }
book-folder-found-line = { $path }. O { -app-name } identificou { $matched } delas pelos dados.
book-folder-unsupported = { $count ->
    [one] Um arquivo que o { -app-name } não consegue abrir
    [many] { $count } de arquivos que o { -app-name } não consegue abrir
   *[other] { $count } arquivos que o { -app-name } não consegue abrir
 }
book-folder-unsupported-line = { $kinds }. Eles continuam na pasta como estão.
book-folder-drm = { $count } EPUB bloqueados com DRM
book-folder-opens = O { -app-name } abre arquivos CBZ, CBR, EPUB e PDF, e pastas de imagens.
book-folder-show-files = Mostrar arquivos
book-folder-add = Adicionar pasta
book-folder-change = Alterar
book-folder-empty = Nada em { $folder } que o { -app-name } consiga abrir
book-folder-empty-line = { $files ->
    [one] Há um arquivo, que não é CBZ, CBR, EPUB, PDF nem imagem.
    [many] Há { $files } de arquivos, nenhum deles CBZ, CBR, EPUB, PDF ou imagens.
   *[other] Há { $files } arquivos, nenhum deles CBZ, CBR, EPUB, PDF ou imagens.
 }
book-folder-choose-other = Escolher outra pasta
store-join-title = Entrar em uma { -plugin-store }
store-invite-field = Convite
store-joined-title = Você entrou em { $store }
store-joined-line = { $count ->
    [one] { $owner } oferece um { -plugin }. Entrar não instala nada; instale aqui ou depois.
    [many] { $owner } oferece { $count } de { -plugin-pl }. Entrar não instala nada; escolha os que quiser aqui ou depois.
   *[other] { $owner } oferece { $count } { -plugin-pl }. Entrar não instala nada; escolha os que quiser aqui ou depois.
 }
store-group-reading = Para ler
store-group-watching = Para assistir
store-install = Instalar
store-installed-row = Instalado
store-install-all-reading = Instalar todos para ler
store-installed = { $count ->
    [one] Um { -plugin } instalado. Ele está em Suas { -source-pl }.
    [many] { $count } de { -plugin-pl } instalados. Eles estão em Suas { -source-pl }.
   *[other] { $count } { -plugin-pl } instalados. Eles estão em Suas { -source-pl }.
 }
store-other-medium = Todo { -plugin } em { $store } é para assistir
store-other-medium-line = Assistir está desativado neste { -device }, então nenhum aparece em Suas { -source-pl }.
store-turn-on-watching = Ativar assistir
store-empty = { $store } ainda não tem { -plugin-pl }
store-empty-line = Você é membro, então novos { -plugin-pl } aparecem em { -plugin-store-cap-pl } quando { $owner } os adiciona.
store-err-unreachable = Não foi possível acessar { $host }
store-err-unreachable-line = O convite parece certo, mas a { -store } não está respondendo. Tente de novo mais tarde ou pergunte a { $owner } se ela está funcionando.
store-other-medium-more = { $count ->
    [one] Mais um é para assistir, o que está desativado neste { -device }.
    [many] Mais { $count } de são para assistir, o que está desativado neste { -device }.
   *[other] Mais { $count } são para assistir, o que está desativado neste { -device }.
 }
store-tv-reading-only = { $count ->
    [one] Mais um é para leitura, o que esta TV não faz.
    [many] Mais { $count } de { -plugin-pl } são para leitura, o que esta TV não faz.
   *[other] Mais { $count } são para leitura, o que esta TV não faz.
 }
store-tv-all-reading = Todos os { -plugin-pl } de { $store } são para leitura, o que esta TV não faz.
store-plugin-row = Versão { $version } · pode contatar { $hosts }
source-added = Adicionado: { $name } · { $series } séries
source-added-plugins = { $count ->
    [one] Um { -plugin } instalado de { $store }
    [many] { $count } de { -plugin-pl } instalados de { $store }
   *[other] { $count } { -plugin-pl } instalados de { $store }
 }
source-row-server = { $libraries ->
    [one] { $kind } · { $series } séries · adicionado agora há pouco
    [many] { $kind } · { $series } séries em { $libraries } de bibliotecas · adicionado agora há pouco
   *[other] { $kind } · { $series } séries em { $libraries } bibliotecas · adicionado agora há pouco
 }
source-row-server-empty = { $kind } · ainda sem séries · confere de novo em 30 minutos
source-row-folder = { $skipped ->
    [one] { $path } · { $series } séries · um arquivo ignorado
    [many] { $path } · { $series } séries · { $skipped } de arquivos ignorados
   *[other] { $path } · { $series } séries · { $skipped } arquivos ignorados
 }
source-row-plugin = De { $store } · { $medium } · versão { $version }
