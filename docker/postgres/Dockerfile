FROM postgres:17

# pg_partman requires the header files that come with the non-alpine image.
# Install the extension package and the pg_cron package (used by Lago).
RUN apt-get update -qq \
 && apt-get install -y --no-install-recommends \
      postgresql-17-partman \
 && rm -rf /var/lib/apt/lists/*
