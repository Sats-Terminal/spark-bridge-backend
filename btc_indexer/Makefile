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

## Build indexer docker image
docker/build:
	docker build -f ./infrastructure/Dockerfile ./../ -t btc_indexer:image

## Open indexer from docker
docker/up:
	docker-compose --env-file ./.env -f ./infrastructure/docker-compose.yml up -d --build

## Close indexer from docker
docker/down:
	docker-compose --env-file ./.env -f ./infrastructure/docker-compose.yml down btc_indexer

## Open db from docker
docker/db/up:
	docker compose --env-file ./.env -f ./infrastructure/docker-compose.yml up persistent-db-storage -d

## Close db from docker
docker/db/down:
	docker compose --env-file ./.env -f ./infrastructure/docker-compose.yml down  -v persistent-db-storage

## Format code
fmt:
	cargo +nightly fmt --all

## Cleans whole directory from temporary files
clean:
	cargo clean --manifest-path $(CORE_CARGO_TOML)