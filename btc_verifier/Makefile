## This help screen
help:
	@printf "Available targets:\n\n"
	@awk '/^[a-zA-Z\-\_0-9%:\\]+/ { \
          helpMessage = match(lastLine, /^## (.*)/); \
          if (helpMessage) { \
            helpCommand = $$1; \
            helpMessage = substr(lastLine, RSTART + 3, RLENGTH); \
      gsub("\\\\", "", helpCommand); \
      gsub(":+$$", "", helpCommand); \
            printf "  \x1b[32;01m%-35s\x1b[0m %s\n", helpCommand, helpMessage; \
          } \
        } \
        { lastLine = $$0 }' $(MAKEFILE_LIST) | sort -u
	@printf "\n"

docker/build:
	docker build . -t spark_test_helper:image

## Open db from docker
docker/up:
	docker-compose -f ./insrastructure/docker-compose.yml up -d --build

## Close db from docker
docker/down:
	docker-compose down -f ./insrastructure/docker-compose.yml spark-helper-service

## Open db from docker
docker/db/up:
	docker compose up -f ./insrastructure/docker-compose.yml persistent-db-storage -d

## Close db from docker
docker/db/down:
	docker compose down -f ./insrastructure/docker-compose.yml -v persistent-db-storage

## Format code
fmt:
	cargo +nightly fmt --all

## Cleans whole directory from temporary files
clean:
	cargo clean --manifest-path $(CORE_CARGO_TOML)