ALTER TABLE metric_hour ADD COLUMN histogram BIGINT[] NOT NULL DEFAULT ARRAY[0,0,0,0,0,0,0,0,0,0]::BIGINT[]
    CHECK (array_ndims(histogram) = 1 AND array_lower(histogram, 1) = 1 AND array_length(histogram, 1) = 10);
