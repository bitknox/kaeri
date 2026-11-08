progress-server-saved = Salvo em { $server }
progress-server-sending = Enviando para { $server }…
progress-server-offline = Vai para { $server } quando você voltar online
progress-server-retrying = { $server } não respondeu. O { -app-name } vai tentar de novo.
progress-server-failed = Não salvo em { $server }: { $reason }.
progress-why-key = o servidor recusou a chave
progress-why-password = o servidor recusou a senha
progress-why-gone = esta série não está mais no servidor
progress-why-gave-up = sem resposta há um dia
progress-row-saved = Progresso salvo { $ago }
progress-row-sending = { $count ->
    [one] Enviando 1 capítulo…
    [many] Enviando { $count } capítulos…
   *[other] Enviando { $count } capítulos…
 }
progress-row-offline = { $count ->
    [one] 1 capítulo será enviado quando você voltar online
    [many] { $count } capítulos serão enviados quando você voltar online
   *[other] { $count } capítulos serão enviados quando você voltar online
 }
progress-row-retrying = { $count ->
    [one] 1 capítulo aguardando. O { -app-name } vai tentar de novo.
    [many] { $count } capítulos aguardando. O { -app-name } vai tentar de novo.
   *[other] { $count } capítulos aguardando. O { -app-name } vai tentar de novo.
 }
progress-row-failed = { $count ->
    [one] 1 capítulo não salvo: { $reason }
    [many] { $count } capítulos não salvos: { $reason }
   *[other] { $count } capítulos não salvos: { $reason }
 }
progress-row-off = O progresso fica neste { $kind }
