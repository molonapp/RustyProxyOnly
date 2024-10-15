#!/bin/bash

PORTS_FILE="/opt/rustymanager/ports"

# Função para verificar se uma porta está em uso
is_port_in_use() {
    local port=$1
    if lsof -i :$port > /dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Função para abrir uma porta de proxy
add_proxy_port() {
    local port=$1
    local status=${2:-"@RustyProxy"}

    if is_port_in_use $port; then
        echo "A porta $port já está em uso."
        return
    fi

    local command="/opt/rustymanager/proxy --port $port --status $status"
    local service_file_path="/etc/systemd/system/proxy${port}.service"
    local service_file_content="[Unit]
Description=RustyProxy${port}
After=network.target

[Service]
LimitNOFILE=infinity
LimitNPROC=infinity
LimitMEMLOCK=infinity
LimitSTACK=infinity
LimitCORE=infinity
LimitAS=infinity
LimitRSS=infinity
LimitCPU=infinity
LimitFSIZE=infinity
Type=simple
ExecStart=${command}
Restart=always

[Install]
WantedBy=multi-user.target"

    echo "$service_file_content" | sudo tee "$service_file_path" > /dev/null
    sudo systemctl daemon-reload
    sudo systemctl enable "proxy${port}.service"
    sudo systemctl start "proxy${port}.service"

    # Salvar a porta no arquivo
    echo $port >> "$PORTS_FILE"
    echo "Porta $port aberta com sucesso."
}

# Função para fechar uma porta de proxy
del_proxy_port() {
    local port=$1

    sudo systemctl disable "proxy${port}.service"
    sudo systemctl stop "proxy${port}.service"
    sudo rm -f "/etc/systemd/system/proxy${port}.service"
    sudo systemctl daemon-reload

    # Remover a porta do arquivo
    sed -i "/^$port$/d" "$PORTS_FILE"
    echo "Porta $port fechada com sucesso."
}

# Função para exibir o menu
show_menu() {
    echo "1) Abrir porta de proxy"
    echo "2) Fechar porta de proxy"
    echo "3) Listar portas abertas"
    echo "4) Sair"
    read -p "Escolha uma opção: " option

    case $option in
        1)
            read -p "Informe a porta a ser aberta: " port
            read -p "Informe o status (opcional): " status
            add_proxy_port $port "$status"
            ;;
        2)
            read -p "Informe a porta a ser fechada: " port
            del_proxy_port $port
            ;;
        3)
            echo "Portas abertas:"
            cat "$PORTS_FILE"
            ;;
        4)
            exit 0
            ;;
        *)
            echo "Opção inválida."
            ;;
    esac
}

# Verificar se o arquivo de portas existe, caso contrário, criar
if [ ! -f "$PORTS_FILE" ]; then
    sudo touch "$PORTS_FILE"
fi

# Loop do menu
while true; do
    show_menu
done
