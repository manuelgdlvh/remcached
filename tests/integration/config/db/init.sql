--
-- PostgreSQL database dump
--

-- Dumped from database version 16.4 (Debian 16.4-1.pgdg120+1)
-- Dumped by pg_dump version 16.4 (Debian 16.4-1.pgdg120+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: game; Type: SCHEMA; Schema: -; Owner: postgres
--

CREATE SCHEMA game;


ALTER SCHEMA game OWNER TO postgres;



--
-- Name: pg_stat_statements; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_stat_statements WITH SCHEMA public;


--
-- Name: EXTENSION pg_stat_statements; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION pg_stat_statements IS 'track planning and execution statistics of all SQL statements executed';


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: game; Type: TABLE; Schema: game; Owner: postgres
--

CREATE TABLE game.game (
    game_id bigint NOT NULL,
    name character varying
);


ALTER TABLE game.game OWNER TO postgres;


--
-- Data for Name: game; Type: TABLE DATA; Schema: game; Owner: postgres
--


INSERT INTO game.game(game_id, name) values(47530, 'The Legend of Zelda: Ocarina of Time 3D');
INSERT INTO game.game(game_id, name) values(47557, 'The Last of Us');
INSERT INTO game.game(game_id, name) values(4755777, 'The Last of Us 2');
INSERT INTO game.game(game_id, name) values(4755771, 'The Last of Us 3');
INSERT INTO game.game(game_id, name) values(47573, 'American Truck Simulator');
INSERT INTO game.game(game_id, name) values(47577, 'Euro Truck Simulator 2');





--
-- Name: game game_pkey; Type: CONSTRAINT; Schema: game; Owner: postgres
--

ALTER TABLE ONLY game.game
    ADD CONSTRAINT game_pkey PRIMARY KEY (game_id);



--
-- Name: SCHEMA public; Type: ACL; Schema: -; Owner: pg_database_owner
--

REVOKE USAGE ON SCHEMA public FROM PUBLIC;


--
-- PostgreSQL database dump complete
--

