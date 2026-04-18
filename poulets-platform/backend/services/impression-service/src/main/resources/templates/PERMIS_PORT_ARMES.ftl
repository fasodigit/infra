<#-- Permis de Port d'Armes - Burkina Faso -->
<#-- Template FreeMarker pour generation PDF via Flying Saucer (ITextRenderer) -->
<#-- Spec ARMES-07 — layout table-based, CSS2 only, system fonts -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"
  "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">
<html xmlns="http://www.w3.org/1999/xhtml" lang="fr">
<head>
    <meta charset="UTF-8"/>
    <title>Permis de Port d'Armes</title>
    <style type="text/css">
        @page {
            size: A4;
            margin: 20mm 18mm 20mm 18mm;
        }

        body {
            font-family: 'Times New Roman', Times, serif;
            margin: 0;
            padding: 0;
            color: #1a1a1a;
            font-size: 12px;
        }

        /* ===== CONTENEUR PRINCIPAL ===== */
        .container {
            border: 3px solid #006B3F;
            padding: 20px 22px;
        }

        .inner-border {
            border: 1px solid #D4A843;
            padding: 18px 20px;
        }

        /* ===== MICRO-TEXT SECURITY STRIP ===== */
        .micro-strip {
            font-size: 4px;
            color: #e0e0e0;
            letter-spacing: 2px;
            text-align: center;
            margin-bottom: 2px;
            overflow: hidden;
            height: 6px;
            line-height: 6px;
        }

        /* ===== EN-TETE ===== */
        .header-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-bottom: 8px;
        }

        .header-table td {
            border: none;
            vertical-align: top;
            padding: 0;
        }

        .header-left {
            text-align: center;
            font-size: 11px;
            line-height: 1.5;
        }

        .country-name {
            font-weight: bold;
            font-size: 16px;
            color: #006B3F;
            letter-spacing: 3px;
        }

        .motto {
            font-style: italic;
            font-size: 11px;
            color: #D4A843;
            font-weight: bold;
        }

        .ministry {
            font-size: 9px;
            font-weight: bold;
            color: #333;
            line-height: 1.4;
            margin-top: 6px;
        }

        /* ===== SEPARATEUR DECORATIF ===== */
        .separator {
            border-top: 2px solid #006B3F;
            border-bottom: 1px solid #D4A843;
            height: 4px;
            margin: 10px 0;
        }

        .separator-thin {
            border-top: 1px solid #D4A843;
            margin: 8px 0;
        }

        /* ===== TITRE ===== */
        .title-section {
            text-align: center;
            margin: 8px 0 12px 0;
        }

        .title-section h1 {
            margin: 4px 0;
            font-size: 18px;
            letter-spacing: 4px;
            color: #006B3F;
            text-decoration: underline;
        }

        .numero-permis {
            font-size: 13px;
            font-weight: bold;
            color: #333;
            margin-top: 4px;
        }

        .numero-demande {
            font-size: 10px;
            color: #666;
        }

        /* ===== SECTION IDENTITE (photo + infos) ===== */
        .identity-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin: 6px 0;
        }

        .identity-table td {
            border: none;
            vertical-align: top;
            padding: 2px 4px;
        }

        .photo-cell {
            width: 100px;
            text-align: center;
            vertical-align: top;
            padding-right: 12px;
        }

        .photo-box {
            width: 90px;
            height: 110px;
            border: 1px solid #006B3F;
            text-align: center;
            font-size: 8px;
            color: #999;
            padding-top: 45px;
        }

        .photo-box img {
            width: 88px;
            height: 108px;
        }

        .info-cell {
            vertical-align: top;
        }

        /* ===== CHAMPS ===== */
        .fields-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
        }

        .fields-table td {
            border: none;
            padding: 2px 4px;
            font-size: 11px;
            line-height: 1.5;
        }

        .field-label {
            font-weight: bold;
            text-transform: uppercase;
            font-size: 9px;
            color: #006B3F;
            width: 130px;
            white-space: nowrap;
            vertical-align: top;
        }

        .field-value {
            border-bottom: 1px dotted #999;
            padding-left: 6px;
            padding-bottom: 1px;
            font-size: 12px;
            color: #000;
        }

        /* ===== SECTION ARME ===== */
        .section-title {
            font-weight: bold;
            font-size: 12px;
            color: #006B3F;
            text-transform: uppercase;
            letter-spacing: 2px;
            margin: 6px 0 4px 0;
            padding: 3px 8px;
            background-color: #f5f5f0;
            border-left: 3px solid #D4A843;
        }

        .arme-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin: 4px 0;
        }

        .arme-table td {
            border: none;
            padding: 2px 4px;
            font-size: 11px;
            line-height: 1.5;
        }

        /* ===== MOTIF ===== */
        .motif-box {
            border: 1px solid #ccc;
            padding: 6px 10px;
            font-size: 11px;
            font-style: italic;
            margin: 4px 0;
            color: #333;
        }

        /* ===== DATES ===== */
        .dates-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin: 6px 0;
        }

        .dates-table td {
            border: none;
            padding: 3px 8px;
            font-size: 12px;
        }

        .date-label {
            font-weight: bold;
            font-size: 10px;
            color: #006B3F;
            text-transform: uppercase;
            width: 180px;
        }

        .date-value {
            font-weight: bold;
            font-size: 13px;
            color: #000;
        }

        .validity-note {
            font-size: 9px;
            font-style: italic;
            color: #666;
        }

        /* ===== DELIVRANCE ===== */
        .delivery-section {
            text-align: center;
            font-size: 12px;
            margin: 8px 0;
        }

        /* ===== FOOTER SIGNATURE ===== */
        .footer-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-top: 15px;
        }

        .footer-table td {
            border: none;
            vertical-align: top;
            padding: 0;
        }

        .stamp-box {
            border: 1px solid #006B3F;
            width: 130px;
            height: 70px;
            text-align: center;
            font-size: 9px;
            color: #006B3F;
            padding-top: 18px;
        }

        .signature-section {
            text-align: center;
        }

        .certify-text {
            font-style: italic;
            font-size: 10px;
            color: #333;
            margin-bottom: 8px;
        }

        .signature-title {
            font-weight: bold;
            font-size: 11px;
            margin-bottom: 4px;
        }

        .signature-name {
            font-weight: bold;
            font-size: 13px;
            margin-top: 25px;
        }

        /* ===== QR CODE VERIFICATION ===== */
        .qr-footer {
            margin-top: 12px;
            border-top: 1px solid #D4A843;
            padding-top: 6px;
        }

        .qr-footer-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
        }

        .qr-footer-table td {
            border: none;
            vertical-align: middle;
            padding: 2px;
        }

        .qr-text {
            font-size: 7px;
            color: #666;
        }

        .qr-text-bold {
            font-size: 7px;
            color: #006B3F;
            font-weight: bold;
        }

        /* ===== NUMERO SERIE ANTI-CONTREFACON ===== */
        .serial-section {
            text-align: right;
            font-size: 7px;
            color: #999;
            margin-top: 4px;
            font-family: 'Courier New', Courier, monospace;
        }

        /* ===== WATERMARK ===== */
        .watermark {
            font-size: 5px;
            color: #f0f0f0;
            text-align: center;
            letter-spacing: 8px;
            margin: 2px 0;
            overflow: hidden;
            height: 7px;
            line-height: 7px;
        }

        /* ===== LOCALISATION HEADER ===== */
        .loc-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-bottom: 4px;
        }

        .loc-table td {
            border: none;
            padding: 1px 4px;
            font-size: 10px;
            color: #333;
        }

        .loc-label {
            font-weight: bold;
            font-size: 9px;
            width: 80px;
        }

        .loc-value {
            font-weight: bold;
            font-size: 10px;
        }
    </style>
</head>
<body>

<div class="container">

    <!-- ===== MICRO-TEXT SECURITY STRIP (top) ===== -->
    <div class="micro-strip">
        REPUBLIQUE-DU-BURKINA-FASO-PERMIS-PORT-ARMES-DOCUMENT-OFFICIEL-SECURISE-REPUBLIQUE-DU-BURKINA-FASO-PERMIS-PORT-ARMES-DOCUMENT-OFFICIEL-SECURISE-REPUBLIQUE-DU-BURKINA-FASO
    </div>

    <div class="inner-border">

        <!-- ==================== EN-TETE ==================== -->
        <table class="header-table">
            <tr>
                <td style="width: 100%; text-align: center;">
                    <div class="country-name">BURKINA FASO</div>
                    <div class="motto">Unit&#233; &#8212; Progr&#232;s &#8212; Justice</div>
                    <div class="ministry">
                        MINIST&#200;RE DE L'ADMINISTRATION TERRITORIALE,<br/>
                        DE LA D&#201;CENTRALISATION ET DE LA S&#201;CURIT&#201;
                    </div>
                </td>
            </tr>
        </table>

        <!-- Localisation administrative -->
        <table class="loc-table">
            <tr>
                <td class="loc-label">R&#233;gion :</td>
                <td class="loc-value">${region!'REGION DU CENTRE'}</td>
                <td class="loc-label">Province :</td>
                <td class="loc-value">${province!'PROVINCE DU KADIOGO'}</td>
            </tr>
            <tr>
                <td class="loc-label">Commune :</td>
                <td class="loc-value" colspan="3">${commune!'OUAGADOUGOU'}</td>
            </tr>
        </table>

        <!-- ==================== SEPARATEUR ==================== -->
        <div class="separator"></div>

        <!-- ==================== TITRE ==================== -->
        <div class="title-section">
            <h1>PERMIS DE PORT D'ARMES</h1>
            <div class="numero-permis">
                N&#176; ${numeroPermis!'______________________'}
            </div>
            <#if numeroDemande??>
            <div class="numero-demande">
                R&#233;f. demande : ${numeroDemande}
            </div>
            </#if>
        </div>

        <!-- ==================== WATERMARK PATTERN ==================== -->
        <div class="watermark">
            BURKINA-FASO-PERMIS-ARMES-BURKINA-FASO-PERMIS-ARMES-BURKINA-FASO-PERMIS-ARMES-BURKINA-FASO-PERMIS-ARMES
        </div>

        <!-- ==================== IDENTITE DU TITULAIRE ==================== -->
        <div class="section-title">Identit&#233; du Titulaire</div>

        <table class="identity-table">
            <tr>
                <!-- Photo d'identite -->
                <td class="photo-cell">
                    <#if photoUrl?? && photoUrl?has_content>
                    <div style="width: 90px; height: 110px; border: 1px solid #006B3F; overflow: hidden;">
                        <img src="${photoUrl}" alt="Photo d'identite" style="width: 88px; height: 108px;"/>
                    </div>
                    <#else>
                    <div class="photo-box">
                        PHOTO<br/>D'IDENTIT&#201;
                    </div>
                    </#if>
                </td>

                <!-- Informations du titulaire -->
                <td class="info-cell">
                    <table class="fields-table">
                        <tr>
                            <td class="field-label">Nom :</td>
                            <td class="field-value">${nom!''}</td>
                        </tr>
                        <tr>
                            <td class="field-label">Pr&#233;nom(s) :</td>
                            <td class="field-value">${prenoms!''}</td>
                        </tr>
                        <tr>
                            <td class="field-label">N&#233;(e) le :</td>
                            <td class="field-value">${dateNaissanceEnLettres!dateNaissance!''}</td>
                        </tr>
                        <tr>
                            <td class="field-label">&#192; :</td>
                            <td class="field-value">${lieuNaissance!''}</td>
                        </tr>
                        <tr>
                            <td class="field-label">Nationalit&#233; :</td>
                            <td class="field-value">${nationalite!'Burkinab&#232;'}</td>
                        </tr>
                        <tr>
                            <td class="field-label">N&#176; CNIB :</td>
                            <td class="field-value">${numeroCnib!''}</td>
                        </tr>
                        <tr>
                            <td class="field-label">Profession :</td>
                            <td class="field-value">${profession!''}</td>
                        </tr>
                        <tr>
                            <td class="field-label">Domicile :</td>
                            <td class="field-value">${domicile!''}</td>
                        </tr>
                        <tr>
                            <td class="field-label">T&#233;l&#233;phone :</td>
                            <td class="field-value">${telephone!''}</td>
                        </tr>
                    </table>
                </td>
            </tr>
        </table>

        <!-- ==================== SEPARATEUR ==================== -->
        <div class="separator-thin"></div>

        <!-- ==================== ARME AUTORISEE ==================== -->
        <div class="section-title">Arme Autoris&#233;e</div>

        <table class="arme-table">
            <tr>
                <td class="field-label" style="width: 130px;">Cat&#233;gorie :</td>
                <td class="field-value">${armeCategorie!''}</td>
                <td class="field-label" style="width: 100px;">Type :</td>
                <td class="field-value">${armeType!''}</td>
            </tr>
            <tr>
                <td class="field-label">Marque :</td>
                <td class="field-value">${armeMarque!'--'}</td>
                <td class="field-label">Mod&#232;le :</td>
                <td class="field-value">${armeModele!'--'}</td>
            </tr>
            <tr>
                <td class="field-label">Calibre :</td>
                <td class="field-value">${armeCalibre!''}</td>
                <td class="field-label">N&#176; S&#233;rie :</td>
                <td class="field-value">${armeNumeroSerie!'&#192; renseigner &#224; l&#39;acquisition'}</td>
            </tr>
        </table>

        <!-- ==================== SEPARATEUR ==================== -->
        <div class="separator-thin"></div>

        <!-- ==================== MOTIF DE LA DEMANDE ==================== -->
        <#if motifDemande?? && motifDemande?has_content>
        <div class="section-title">Motif de la Demande</div>
        <div class="motif-box">
            ${motifDemande}
        </div>
        </#if>

        <!-- ==================== WATERMARK PATTERN ==================== -->
        <div class="watermark">
            DOCUMENT-OFFICIEL-SECURISE-DOCUMENT-OFFICIEL-SECURISE-DOCUMENT-OFFICIEL-SECURISE-DOCUMENT-OFFICIEL-SECURISE
        </div>

        <!-- ==================== DATES DE VALIDITE ==================== -->
        <div class="separator-thin"></div>

        <table class="dates-table">
            <tr>
                <td class="date-label">Date de d&#233;livrance :</td>
                <td class="date-value">${dateDelivrance!'____/____/________'}</td>
                <td class="date-label">Date d'expiration :</td>
                <td class="date-value">${dateExpiration!'____/____/________'}</td>
            </tr>
            <tr>
                <td colspan="4">
                    <span class="validity-note">Ce permis est valable pour une dur&#233;e de cinq (5) ans &#224; compter de la date de d&#233;livrance.</span>
                </td>
            </tr>
        </table>

        <!-- ==================== DELIVRANCE ==================== -->
        <div class="delivery-section">
            D&#233;livr&#233; &#224; <strong>${lieuDelivrance!'Ouagadougou'}</strong>
            <#if dateDelivrance??>, le <strong>${dateDelivrance}</strong></#if>
            <br/>
            <span style="font-size: 11px;">par ${autoriteDelivrance!'Pr&#233;fecture de Ouagadougou'}</span>
        </div>

        <!-- ==================== PIED DE PAGE : SIGNATURE + CACHET ==================== -->
        <table class="footer-table">
            <tr>
                <td style="width: 160px; vertical-align: top;">
                    <div class="stamp-box">
                        CACHET<br/>OFFICIEL
                    </div>
                </td>
                <td style="width: 40%;"> </td>
                <td style="padding-left: 20px;">
                    <div class="signature-section">
                        <div class="certify-text">
                            Certifi&#233; le pr&#233;sent permis conforme<br/>
                            aux dispositions r&#233;glementaires en vigueur.
                        </div>
                        <div class="signature-title">L'Autorit&#233; Comp&#233;tente,</div>
                        <#if signatureUrl?? && signatureUrl?has_content>
                        <div>
                            <img src="${signatureUrl}" alt="Signature" style="width: 120px; height: auto;"/>
                        </div>
                        </#if>
                        <div class="signature-name">${nomOfficier!''}</div>
                    </div>
                </td>
            </tr>
        </table>

        <!-- ==================== QR CODE VERIFICATION ==================== -->
        <#if referenceUnique?? || codeVerification?? || qrCodeUrl??>
        <div class="qr-footer">
            <table class="qr-footer-table">
                <tr>
                    <td style="width: 65%;">
                        <div class="qr-text-bold">DOCUMENT OFFICIEL S&#201;CURIS&#201;</div>
                        <#if referenceUnique??>
                        <div class="qr-text">R&#233;f&#233;rence : ${referenceUnique}</div>
                        </#if>
                        <#if codeVerification??>
                        <div class="qr-text">Code de v&#233;rification : ${codeVerification}</div>
                        </#if>
                        <#if documentHash?? && documentHash?has_content>
                        <div style="font-size: 6pt; color: #666; margin-top: 2px;">Hash: ${documentHash}</div>
                        </#if>
                        <#if numeroPermis??>
                        <div class="qr-text">Permis N&#176; : ${numeroPermis}</div>
                        </#if>
                        <div class="qr-text">V&#233;rification : ${verificationUrl!'https://verify.actes.gov.bf'}</div>
                    </td>
                    <td style="width: 35%; text-align: right;">
                        <#if qrCodeUrl?? && qrCodeUrl?has_content>
                        <img src="${qrCodeUrl}" alt="QR Code" style="width: 70px; height: 70px;"/>
                        <#else>
                        <div style="width: 70px; height: 70px; border: 1px solid #ccc; text-align: center; font-size: 7px; color: #ccc; padding-top: 28px; margin-left: auto;">
                            QR CODE
                        </div>
                        </#if>
                    </td>
                </tr>
            </table>
        </div>
        </#if>

        <!-- ===== MICRO-TEXT SECURITY STRIP (bottom) ===== -->
        <div class="micro-strip">
            PERMIS-PORT-ARMES-BURKINA-FASO-DOCUMENT-OFFICIEL-SECURISE-PERMIS-PORT-ARMES-BURKINA-FASO-DOCUMENT-OFFICIEL-SECURISE-PERMIS-PORT-ARMES-BURKINA-FASO
        </div>

    </div><!-- end inner-border -->

    <!-- ==================== NUMERO SERIE ANTI-CONTREFACON ==================== -->
    <#if numeroSerieDocument??>
    <div class="serial-section">
        S/N: ${numeroSerieDocument}
    </div>
    </#if>

</div><!-- end container -->

</body>
</html>
