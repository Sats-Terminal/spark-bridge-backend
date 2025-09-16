

down_docker_compose() {
    echo "Shutting down docker compose..."
    docker compose -f "./infrastructure/databases.docker-compose.yml" down -v
    echo "Docker compose shut down successfully."
}

shut_down_services() {
    
}

main() {
    down_docker_compose
}

main