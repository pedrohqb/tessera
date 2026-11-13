//! A deliberately small translation layer: English strings in the code, and a
//! table for Brazilian Portuguese, picked from LC_ALL / LC_MESSAGES / LANG.
//! Enough for a handful of screens without a gettext toolchain.

use std::sync::OnceLock;

fn portuguese() -> bool {
    static PT: OnceLock<bool> = OnceLock::new();
    *PT.get_or_init(|| {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(v) = std::env::var(var) {
                if v.is_empty() {
                    continue;
                }
                return v.starts_with("pt");
            }
        }
        false
    })
}

/// Translate a UI string.
pub fn t(s: &'static str) -> &'static str {
    if !portuguese() {
        return s;
    }
    PT_BR
        .iter()
        .find(|(en, _)| *en == s)
        .map(|(_, pt)| *pt)
        .unwrap_or(s)
}

const PT_BR: &[(&str, &str)] = &[
    // Window and menu
    ("Tessera", "Tessera"),
    ("Token hub", "Central de tokens"),
    ("Start automatically at login", "Iniciar automaticamente no login"),
    ("About Tessera", "Sobre o Tessera"),
    ("Restart Tessera", "Reiniciar o Tessera"),
    ("Token JSignPdf opens", "Token que o JSignPdf abre"),
    ("Close JSignPdf, then press Set up: it was set up for another token", "Feche o JSignPdf e aperte Configurar: ele foi configurado para outro token"),
    ("Could not restart Tessera", "Não foi possível reiniciar o Tessera"),
    ("Quit it from the menu and open it again", "Saia pelo menu e abra-o de novo"),
    ("Open the app once, so it makes its settings, then restart Tessera: it sees an app's folders only if they were there when it started", "Abra o aplicativo uma vez, para ele criar as configurações dele, e depois reinicie o Tessera: ele só enxerga as pastas de um aplicativo que já existiam quando ele abriu"),
    ("Quit", "Sair"),
    ("Refresh", "Atualizar"),
    // Service
    ("Service", "Serviço"),
    ("Token service", "Serviço de tokens"),
    ("Running", "Em execução"),
    ("Stopped", "Parado"),
    ("Start", "Iniciar"),
    ("Restart", "Reiniciar"),
    ("Smart card service (pcscd)", "Serviço de cartões (pcscd)"),
    ("Reachable", "Acessível"),
    ("Not reachable: is pcscd installed and running on the system?",
     "Inacessível: o pcscd está instalado e em execução no sistema?"),
    ("Another program already answers on this socket (is Tessera already running, for example as `tessera serve`?).",
     "Outro programa já responde neste soquete (o Tessera já está rodando, por exemplo como `tessera serve`?)."),
    ("The server exited", "O servidor foi encerrado"),
    // Readers / tokens
    ("Readers", "Leitoras"),
    ("No reader found", "Nenhuma leitora encontrada"),
    ("Connect a token or card reader.", "Conecte um token ou uma leitora de cartão."),
    ("Card present", "Cartão presente"),
    ("Empty", "Vazia"),
    ("Tokens", "Tokens"),
    ("No token found", "Nenhum token encontrado"),
    ("Insert a token, or install the driver for it on the Drivers page.",
     "Insira um token ou instale o driver dele na página Drivers."),
    ("certificate", "certificado"),
    ("certificates", "certificados"),
    ("Issued by", "Emitido por"),
    ("Valid until", "Válido até"),
    ("Expired on", "Expirou em"),
    ("No certificates visible before login", "Nenhum certificado visível antes do login"),
    // Drivers
    ("Drivers", "Drivers"),
    ("No driver installed", "Nenhum driver instalado"),
    ("Driver files missing", "Arquivos do driver ausentes"),
    // Apps
    ("Apps", "Aplicativos"),
    ("Copy the command that lets this app reach Tessera",
     "Copiar o comando que permite a este aplicativo acessar o Tessera"),
    ("Command copied", "Comando copiado"),
    ("Set up", "Configurar"),
    ("Could not set up", "Não foi possível configurar"),
    ("driver(s)", "driver(s)"),
    ("Use this driver", "Usar este driver"),
    (
        "Driver on. Apps that were already open see it once reopened.",
        "Driver ligado. Os aplicativos que já estavam abertos passam a vê-lo quando forem reabertos.",
    ),
    (
        "Starts every driver again: the apps that were open lose the tokens until they are reopened",
        "Reinicia todos os drivers: os aplicativos que estavam abertos perdem os tokens até serem reabertos",
    ),
    ("System connection (the system's p11-kit server)", "Conexão com o sistema (servidor p11-kit do sistema)"),
    ("Could not write the host module files", "Não foi possível gravar os arquivos de módulo do sistema"),
    ("A driver could not be read", "Não foi possível ler um driver"),
    ("No steps: the system connection is enough", "Nenhum passo: basta a conexão com o sistema"),
    ("Or by hand in Firefox: Settings → Privacy & Security → Security Devices → Load, module file:",
     "Ou à mão no Firefox: Configurações → Privacidade e Segurança → Dispositivos de segurança → Carregar, arquivo do módulo:"),
    // Supervision, tray and set-up
    ("Serving", "Servindo"),
    ("published", "publicado(s)"),
    ("token(s)", "token(s)"),
    ("On", "Ativo"),
    ("Off: the apps cannot see the tokens. Run once in a terminal:",
     "Desligado: os aplicativos não veem os tokens. Rode uma vez no terminal:"),
    ("token(s) of Tessera seen by the apps", "token(s) do Tessera vistos pelos aplicativos"),
    ("driver(s) published; no token to check yet", "driver(s) publicado(s); nenhum token para conferir ainda"),
    ("The apps do not see Tessera's tokens: what answers at the system connection does not list them (the system's p11-kit server is probably off, and Flatpak's own answers in its place). Run once in a terminal:",
     "Os aplicativos não veem os tokens do Tessera: o que responde na conexão com o sistema não os lista (o servidor p11-kit do sistema provavelmente está desligado, e o do próprio Flatpak responde no lugar dele). Rode uma vez no terminal:"),
    ("Off: the apps cannot see the tokens. Copy the command with the button on the right and run it once in a terminal.",
     "Desligado: os aplicativos não veem os tokens. Copie o comando com o botão à direita e rode-o uma vez no terminal."),
    ("The apps do not see Tessera's tokens: what answers at the system connection does not list them (the system's p11-kit server is probably off, and Flatpak's own answers in its place). Copy the command with the button on the right and run it once in a terminal.",
     "Os aplicativos não veem os tokens do Tessera: o que responde na conexão com o sistema não os lista (o servidor p11-kit do sistema provavelmente está desligado, e o do próprio Flatpak responde no lugar dele). Copie o comando com o botão à direita e rode-o uma vez no terminal."),
    ("On this system signing does not work (old p11-kit); see the project's documentation",
     "Neste sistema a assinatura não funciona (p11-kit antigo); veja a documentação do projeto"),
    ("Documentation", "Documentação"),
    ("https://github.com/pedrohqb/tessera#distributions",
     "https://github.com/pedrohqb/tessera#distribui%C3%A7%C3%B5es"),
    ("Step 2 by hand", "Passo 2 à mão"),
    ("Step 1 missing; step 2 by hand", "Falta o passo 1; o passo 2 é à mão"),
    ("no profile found: open the app once, then try again", "nenhum perfil encontrado: abra o aplicativo uma vez e tente de novo"),
    ("close the app first, then try again", "feche o aplicativo antes e tente de novo"),
    ("if it is closed, a crash may have left the file .lock in its settings folder", "se ele estiver fechado, um travamento pode ter deixado o arquivo .lock na pasta de configurações dele"),
    ("the system's p11-kit server does not list Tessera's tokens: plug in the token, check the system connection, then try again", "o servidor p11-kit do sistema não lista os tokens do Tessera: conecte o token, confira a conexão com o sistema e tente de novo"),
    ("Done: Tessera's module is in every profile", "Feito: o módulo do Tessera está em todos os perfis"),
    ("Done: JSignPdf opens Tessera's tokens (keystore type PKCS11). To sign, type the token's PIN in “Keystore password” and press “Load keys”. Set up again if the token moves to another reader", "Feito: o JSignPdf abre os tokens do Tessera (tipo de keystore PKCS11). Para assinar, digite o PIN do token em “Palavra-passe do keystore” e aperte “Carregar chaves”. Configure de novo se o token mudar de leitora"),
    ("Done: LibreOffice uses a certificate database of its own (Tools → Options → Security → Certificate Path)", "Feito: o LibreOffice usa um banco de certificados próprio (Ferramentas → Opções → Segurança → Caminho do certificado)"),
    ("Close the app first: adds the module to every profile", "Feche o aplicativo antes: acrescenta o módulo a todos os perfis"),
    ("Close LibreOffice first: makes a certificate database of its own and sets it as its certificate path", "Feche o LibreOffice antes: cria um banco de certificados próprio e o define como caminho do certificado"),
    ("Done: Master PDF Editor's PKCS#11 provider is p11-kit (Tools → Settings → Certificates)", "Feito: o provedor PKCS#11 do Master PDF Editor é o p11-kit (Ferramentas → Configurações → Certificados)"),
    ("Close Master PDF Editor first: sets p11-kit as its PKCS#11 provider (Tools → Settings → Certificates)", "Feche o Master PDF Editor antes: define o p11-kit como o provedor PKCS#11 dele (Ferramentas → Configurações → Certificados)"),
    ("Close JSignPdf first: writes its pkcs11.cfg (p11-kit and the slot of Tessera's tokens) and chooses the keystore type PKCS11 in its settings (needs the token plugged in and the system connection)", "Feche o JSignPdf antes: grava o pkcs11.cfg dele (o p11-kit e o slot dos tokens do Tessera) e escolhe o tipo de keystore PKCS11 nas configurações dele (precisa do token conectado e da conexão com o sistema)"),
    ("Open LibreOffice once (it writes its settings when it closes), then come back", "Abra o LibreOffice uma vez (ele grava as configurações ao fechar) e volte aqui"),
    ("Or by hand in Thunderbird: Settings → Privacy & Security → Security Devices → Load, module file:", "Ou à mão no Thunderbird: Configurações → Privacidade e segurança → Dispositivos de segurança → Carregar, arquivo do módulo:"),
    ("Or by hand in Master PDF Editor: Tools → Settings → Certificates → PKCS#11 provider, file:", "Ou à mão no Master PDF Editor: Ferramentas → Configurações → Certificados → Provedor PKCS#11, arquivo:"),
    ("Native app; same certificate database as the Chromium-based browsers", "Aplicativo nativo; o mesmo banco de certificados dos navegadores baseados no Chromium"),
    ("Checking…", "Verificando…"),
    ("Steps 1 and 2 missing", "Faltam os passos 1 e 2"),
    ("step 2 done; restart the app", "passo 2 feito; reinicie o aplicativo"),
    ("Step 1 · access to the system's p11-kit server", "Passo 1 · acesso ao servidor p11-kit do sistema"),
    ("Done: the app can reach the system's p11-kit server", "Feito: o aplicativo chega ao servidor p11-kit do sistema"),
    ("Run once in a terminal, then restart the app:", "Rode uma vez no terminal e reinicie o aplicativo:"),
    ("Step 2 · remove the duplicate module", "Passo 2 · retirar o módulo duplicado"),
    ("Step 2 · certificate database", "Passo 2 · banco de certificados"),
    ("Done: the app has its own certificate database", "Feito: o aplicativo tem o seu banco de certificados"),
    ("An older Tessera added a module that makes the app ask for the PIN twice. Close the app first",
     "Uma versão anterior do Tessera acrescentou um módulo que faz o aplicativo pedir o PIN duas vezes. Feche o aplicativo antes"),
    ("Remove", "Retirar"),
    ("Copy the command that installs this driver", "Copiar o comando que instala este driver"),
    ("It looks like a token for the driver", "Parece um token do driver"),
    ("If it does not show under Tokens, install that driver: from the app store or FlatPark's page (button), or with the command (copy button); then quit Tessera from its menu and open it again", "Se ele não aparecer em Tokens, instale esse driver: pela loja de aplicativos ou pela página do FlatPark (botão), ou pelo comando (botão de copiar); depois saia do Tessera pelo menu e abra de novo"),
    ("Install it from the app store (an add-on on Tessera's page) or FlatPark's page (button), or with the command (copy button); then restart Tessera (menu)", "Instale pela loja de aplicativos (complemento na página do Tessera) ou pela página do FlatPark (botão), ou pelo comando (botão de copiar); depois reinicie o Tessera (menu)"),
    ("Open this driver's page on FlatPark, to install it from the app store", "Abrir a página deste driver no FlatPark, para instalar pela loja de aplicativos"),
    ("Not installed", "Não instalado"),
    ("GD Burti (StarSign) and other SafeSign tokens", "GD Burti (StarSign) e outros tokens SafeSign"),
    ("SafeNet eToken 5100/5110 and IDPrime cards", "SafeNet eToken 5100/5110 e cartões IDPrime"),
    ("Serpro's cloud certificate (no token to connect)", "certificado em nuvem do Serpro (sem token para conectar)"),
    ("these show twice to the apps, because the system has a driver of its own for them too (its OpenSC, for example); switch off Tessera's driver for them on the Drivers page, or remove the system's", "estes aparecem duas vezes para os aplicativos, porque o sistema também tem um driver próprio para eles (o OpenSC do sistema, por exemplo); desligue o driver do Tessera para eles na página Drivers, ou remova o do sistema"),
    ("The same token comes from two drivers", "O mesmo token vem de dois drivers"),
    ("The apps would list it twice: switch one of the two drivers off on the Drivers page", "Os aplicativos o listariam duas vezes: desligue um dos dois drivers na página Drivers"),
    ("not a LibreOffice settings file", "não é um arquivo de configurações do LibreOffice"),
    ("it already has a certificate path of its own, which Tessera keeps. To use Tessera's, clear it in Tools → Options → Security → Certificate Path, then try again", "ele já tem um caminho de certificados próprio, que o Tessera mantém. Para usar o do Tessera, limpe-o em Ferramentas → Opções → Segurança → Caminho do certificado e tente de novo"),
    ("open the app once, then restart Tessera and try again", "abra o aplicativo uma vez, depois reinicie o Tessera e tente de novo"),
    ("Apps that use the tokens through Tessera.",
     "Aplicativos que usam os tokens por meio do Tessera."),
    ("Each needs one or two steps, once: access to the system connection, then (for some) a certificate database.",
     "Cada um precisa de um ou dois passos, uma vez: acesso à conexão com o sistema e, para alguns, um banco de certificados."),
    ("Apps installed from the distribution reach Tessera through p11-kit-proxy.so, without the system connection: each needs it in its certificate database, once.",
     "Os aplicativos instalados pela distribuição chegam ao Tessera pelo p11-kit-proxy.so, sem a conexão com o sistema: cada um precisa dele no seu banco de certificados, uma vez."),
    ("Flatpak", "Flatpak"),
    ("Native", "Nativos"),
    // Native apps
    ("Native app", "Aplicativo nativo"),
    ("Chromium-based browsers", "Navegadores baseados no Chromium"),
    ("Okular and Papers", "Okular e Papers"),
    ("Native apps; they use Firefox's certificate database",
     "Aplicativos nativos; usam o banco de certificados do Firefox"),
    ("Ready: the system's NSS already loads p11-kit",
     "Pronto: o NSS do sistema já carrega o p11-kit"),
    ("Each token shows twice: the system's NSS already loads p11-kit, and Tessera's module is there too. Remove Tessera's",
     "Cada token aparece duas vezes: o NSS do sistema já carrega o p11-kit, e o módulo do Tessera também está lá. Retire o do Tessera"),
    ("Tessera's duplicate module removed; restart the app",
     "módulo duplicado do Tessera retirado; reinicie o aplicativo"),
    ("Set up Firefox first", "Configure o Firefox antes"),
    ("Needs Firefox: install it, open it once and set it up",
     "Precisa do Firefox: instale-o, abra-o uma vez e configure-o"),
    ("Chromium, Chrome, Edge, Brave", "Chromium, Chrome, Edge, Brave"),
    ("Adds p11-kit-proxy.so to the app's certificate database. Close the app first",
     "Acrescenta o p11-kit-proxy.so ao banco de certificados do aplicativo. Feche o aplicativo antes"),
    ("Set-up missing", "Falta configurar"),
    ("Open the app once, then come back", "Abra o aplicativo uma vez e volte aqui"),
    ("Not available: the Chromium or Evolution Flatpak uses the same certificate database (~/.pki/nssdb) and would see each token twice",
     "Indisponível: o Chromium ou o Evolution em Flatpak usa o mesmo banco de certificados (~/.pki/nssdb) e veria cada token duas vezes"),
    ("set up; restart the app", "configurado; reinicie o aplicativo"),
    ("Open PJeOffice Pro once (it writes ~/.pjeoffice-pro/pjeoffice-pro.config when it closes), then come back",
     "Abra o PJeOffice Pro uma vez (ele grava o ~/.pjeoffice-pro/pjeoffice-pro.config ao fechar) e volte aqui"),
    ("Points PJeOffice Pro's list of A3 libraries (list.a3) at p11-kit's library. Close PJeOffice Pro first",
     "Aponta a lista de bibliotecas A3 do PJeOffice Pro (list.a3) para a biblioteca do p11-kit. Feche o PJeOffice Pro antes"),
    ("Show an icon in the tray", "Mostrar ícone na bandeja"),
    ("Ready", "Pronto"),
    ("Step 1 missing", "Falta o passo 1"),
    ("Step 2 missing", "Falta o passo 2"),
    ("Makes the certificate database the app reads when signing",
     "Cria o banco de certificados que o aplicativo lê ao assinar"),
    ("Open Tessera", "Abrir o Tessera"),
    ("Quit (stop serving the tokens)", "Sair (parar de servir os tokens)"),
    ("Switched off", "Desligado"),
    ("Published: the apps can use it", "Publicado: os aplicativos podem usar"),
    ("No answer; trying again", "Sem resposta; tentando de novo"),
    ("of", "de"),
    ("Starting…", "Iniciando…"),
    ("No answer 3 times in a row. Press Restart to try again, or switch it off.",
     "Sem resposta 3 vezes seguidas. Aperte Reiniciar para tentar de novo, ou desligue o driver."),
    ("Could not turn on starting at login", "Não foi possível ativar o início automático"),
    // Web signers (signer_rows.rs)
    ("Web signers", "Assinadores web"),
    ("Browser plugins that sign on websites (Web PKI, WebSigner), installed as add-ons of Tessera. The Flatpak browsers run them: each needs one command, once per plugin.", "Plugins de navegador que assinam em sites (Web PKI, WebSigner), instalados como complementos do Tessera. Os navegadores em Flatpak os executam: cada um precisa de um comando, uma vez por plugin."),
    ("Done: the browser can start it (restart the browser if it was open).", "Feito: o navegador consegue iniciá-lo (reinicie o navegador se estava aberto)."),
    ("In the plugin's settings, tick", "Nas configurações do plugin, marque"),
    ("once.", "uma vez."),
    ("Run once in a terminal (copy button), then restart the browser.", "Rode uma vez num terminal (botão de copiar) e reinicie o navegador."),
    ("First do step 1 of this browser (Apps section). Then run once in a terminal (copy button) and restart the browser.", "Primeiro faça o passo 1 deste navegador (seção Aplicativos). Depois rode uma vez num terminal (botão de copiar) e reinicie o navegador."),
    ("Step 1 missing (Apps section)", "Falta o passo 1 (seção Aplicativos)"),
    ("set up", "configurados"),
    ("Copy the command that lets this browser start the signer", "Copiar o comando que deixa este navegador iniciar o assinador"),
    ("No Flatpak browser installed", "Nenhum navegador em Flatpak instalado"),
    ("Firefox, Chromium, Google Chrome, Microsoft Edge or Brave, from Flathub", "Firefox, Chromium, Google Chrome, Microsoft Edge ou Brave, do Flathub"),
    ("Open this signer's page on FlatPark, to install it from the app store", "Abrir a página deste assinador no FlatPark, para instalá-lo pela loja de aplicativos"),
    ("Copy the command that installs this signer", "Copiar o comando que instala este assinador"),
    ("Signing sites that use the Web PKI plugin", "Sites de assinatura que usam o plugin Web PKI"),
    ("e-SAJ and other Softplan systems", "e-SAJ e outros sistemas da Softplan"),
    ("Certisign's signing portals", "Portais de assinatura da Certisign"),
    // The window's pages (window.rs)
    ("Home", "Início"),
    ("System", "Sistema"),
    ("Menu", "Menu"),
    ("System connection", "Conexão com o sistema"),
    ("No token connected", "Nenhum token conectado"),
    ("token found", "token encontrado"),
    ("tokens found", "tokens encontrados"),
    ("Everything works", "Tudo funcionando"),
    ("Needs attention", "Precisa de atenção"),
    ("See", "Ver"),
    ("Open the System page", "Abrir a página Sistema"),
    ("Startup", "Inicialização"),
    ("Installed", "Instalados"),
    ("Available to install", "Disponíveis para instalar"),
    (
        "Vendors' drivers, as add-ons of Tessera: install the one for your token.",
        "Drivers dos fabricantes, como complementos do Tessera: instale o do seu token.",
    ),
    (
        "Recommended: it hangs some tokens (GD Burti) and shows others twice",
        "Recomendado: ele trava alguns tokens (GD Burti) e mostra outros em dobro",
    ),
    // The system's OpenSC (window.rs)
    ("Switch off the system's OpenSC", "Desligar o OpenSC do sistema"),
    ("Recommended. If the system has OpenSC of its own, it hangs some tokens (GD Burti) and shows the others twice; Tessera's OpenSC serves the same cards. Stays off with Tessera closed. Restart the apps that were open.", "Recomendado. Se o sistema tiver um OpenSC próprio, ele trava alguns tokens (GD Burti) e mostra os outros em dobro; o OpenSC do Tessera atende os mesmos cartões. Continua desligado com o Tessera fechado. Reinicie os aplicativos que estavam abertos."),
    ("Could not change the system's OpenSC", "Não foi possível mudar o OpenSC do sistema"),
];

#[cfg(test)]
mod tests {
    use super::PT_BR;
    use std::collections::HashSet;

    /// The .rs files under `dir`, subfolders (src/ui/) included.
    fn sources(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(sources(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
        files
    }

    /// Every string the code passes to `t(…)`, read from the sources.
    fn ui_strings() -> Vec<(String, String)> {
        let mut found = Vec::new();
        for path in sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")) {
            if path.ends_with("i18n.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let bytes = src.as_bytes();
            let mut i = 0;
            while let Some(at) = src[i..].find("t(") {
                let start = i + at;
                i = start + 2;
                // A call to `t`, not to `print(`, `get(`, …
                if start > 0
                    && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_')
                {
                    continue;
                }
                let rest = src[i..].trim_start();
                let Some(lit) = rest.strip_prefix('"') else {
                    continue;
                };
                let mut s = String::new();
                let mut chars = lit.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(n) = chars.next() {
                                s.push(match n {
                                    'n' => '\n',
                                    other => other,
                                });
                            }
                        }
                        c => s.push(c),
                    }
                }
                let file = path.file_name().unwrap().to_string_lossy().into_owned();
                found.push((file, s));
            }
        }
        found
    }

    #[test]
    fn every_ui_string_has_a_portuguese_translation() {
        let keys: HashSet<&str> = PT_BR.iter().map(|(en, _)| *en).collect();
        let missing: Vec<String> = ui_strings()
            .into_iter()
            .filter(|(_, s)| !keys.contains(s.as_str()))
            .map(|(f, s)| format!("{f}: {s:?}"))
            .collect();
        assert!(
            missing.is_empty(),
            "no translation for:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn no_string_is_translated_twice() {
        let mut seen = HashSet::new();
        let dups: Vec<&str> = PT_BR
            .iter()
            .map(|(en, _)| *en)
            .filter(|en| !seen.insert(*en))
            .collect();
        assert!(dups.is_empty(), "translated more than once: {dups:?}");
    }
}
