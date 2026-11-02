-- Recherche sans tenir compte des majuscules ni des accents : même règle que
-- `classes::fold` côté Rust (un caractère accentué → sa lettre de base).
CREATE OR REPLACE FUNCTION looty_fold(t TEXT) RETURNS TEXT
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT translate(lower(coalesce(t, '')), 'àáâãäåçèéêëìíîïñòóôõöùúûüýÿœæ', 'aaaaaaceeeeiiiinooooouuuuyyoa')
$$;
